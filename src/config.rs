use std::{
	path::PathBuf,
	process,
	io::{self, Read, Write},
	sync::OnceLock,
	error::Error,
	str::FromStr,
	fmt::{Debug, Display},
	fs,
};
use chrono::{Local, format::StrftimeItems, DateTime, Offset};
use clap::{
	builder::{Styles, styling::{AnsiColor, Effects}},
	crate_description,
	crate_authors,
	crate_name,
	Parser,
};
use colored::Colorize;
use dirs::config_dir;
use regex::{Regex, Captures};
use sysinfo::System;
use toml::{value::Array, Table};
use crate::{default_configs, handle_error};

macro_rules! parse_config_field {
	($name:ident.$i1:ident.$i2:ident) => {{
		const I1: &'static str = stringify!($i1);
		const I2: &'static str = stringify!($i2);
		$name
			.get(I1)
			.ok_or(format!("Missing table {} in configuration file!", I1))?
			.get(I2)
			.ok_or(format!("Could not find {} in configuration file!", I2))?
	}};
	($name:ident.$i1:ident.$i2:ident -> $type:ident) => {
		parse_config_field!($name.$i1.$i2).clone().try_into::<$type>()?
	};
	($name:ident.$i1:ident.$i2:ident -> map!($err:literal, value.$as:ident() -> $f:expr)) => {
		parse_config_field!($name.$i1.$i2 -> Array)
			.iter()
			.map(|value| {
				map!(value, $err, value.$as() -> $f)
			})
			.collect()
	};
}

macro_rules! map {
	($name:expr, $err:literal, value.$as:ident() -> $f:expr) => {
		$name
			.$as()
			.map_or_else(|| Err($err), $f)
			.unwrap_or_else(handle_error)
	};
}

#[derive(Parser)]
#[command(
	version,
	styles = Styles::plain()
		.header(AnsiColor::Green.on_default().effects(Effects::BOLD))
		.usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
		.literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
		.placeholder(AnsiColor::Cyan.on_default())
		.error(AnsiColor::Red.on_default().effects(Effects::BOLD))
		.valid(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
		.invalid(AnsiColor::Yellow.on_default().effects(Effects::BOLD)),
	about = format!("{}\nMade by {}", crate_description!(), crate_authors!()),
	long_about = None
)]
struct Cli {
    /// Path to the configuration file
    #[arg(
		short,
		long,
		default_value(match config_dir() {
			Some(mut dir) => {
				dir.push(crate_name!());
				dir.push("config.toml");
				dir.into_os_string()
			},
			None => "config.toml".into(),
		})
	)]
    config_path: PathBuf,

	/// Don't print anything.
	#[arg(short, long)]
	quiet: bool,
}

struct Item<'a> (chrono::format::Item<'a>);

impl Display for Item<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		static DATE: OnceLock<DateTime<Local>> = OnceLock::new();
		let date = DATE.get_or_init(Local::now);
		let offset = date.offset();
		chrono::format::format_item(
			f,
			Some(&date.date_naive()),
			Some(&date.time()),
			Some(&(offset.to_string(), offset.fix())),
			&self.0
		)
	}
}

#[derive(Debug)]
enum TagKeepMode {
	/// Keep the tagged folder with only the tag inside.
	Tag,

	/// Keep the tagged folder with no file inside.
	Dir,

	/// Don't keep the tagged folder at all.
	None,
}

#[derive(Debug)]
pub struct Config {
	paths: Vec<PathBuf>,
	exclude: Vec<Regex>,
	exclude_tags: Vec<(String, TagKeepMode)>,
	name: String,
	level: u32,
	memlimit: u64,
	threads: u32,
	block_size: u64,
}

pub static CONFIG: OnceLock<Config> = OnceLock::new();

fn parse_excluded_tag(value: &Array) -> Result<(String, TagKeepMode), &'static str> {
	if value.len() != 2 {
		return Err("an excluded tag should only be the name and mode");
	}
	Ok((
		map!(
			value[0],
			"excluded tag names must be strings",
			value.as_str() -> |s| Ok(String::from(s))
		),
		map!(
			value[1],
			"excluded tag modes must be strings",
			value.as_str() -> |s| Ok(match s.to_ascii_lowercase().as_str() {
				"keep-tag" | "keep tag" | "keep_tag" | "keeptag" => TagKeepMode::Tag,
				"keep-dir" | "keep dir" | "keep_dir" | "keepdir" => TagKeepMode::Dir,
				"keep-none" | "keep none" | "keep_none" | "keepnone" => TagKeepMode::None,
				_ => return Err("unknown tag mode"),
			})
		),
	))
}

fn parse_name_capture(caps: &Captures) -> String {
	if let Some(group) = caps.get(1) {
		let result = match group.as_str() {
			"!hostname" => Some(System::host_name().unwrap_or_else(|| String::from("unknown"))),
			_ => None,
		};
		if let Some(result) = result {
			match caps.get(2) {
				Some(end) => result + end.as_str(),
				None => result
			}
		} else {
			let invalid = String::from("%") + group.as_str();
			handle_error(format!("unknown specifier '{}'", invalid.yellow().bold()))
		}
	} else {
		let captured = caps.get(0).unwrap().as_str();
		let mut result = String::new();
		let mut iter = StrftimeItems::new(captured);
		while let Some(item) = iter.next() {
			use chrono::format::Item::Error;
			if item == Error {
				let mut suffix = String::new();
				for item in iter {
					if item != Error {
						suffix += &Item(item).to_string()
					}
				}
				return handle_error(format!(
					"unknown specifier '{}'",
					captured.strip_suffix(&suffix).unwrap_or(captured).yellow().bold()
				));
			} else {
				result += &Item(item).to_string();
			}
		}
		result
	}
}

pub fn init() -> Result<(), Box<dyn Error>> {
	let cli = Cli::parse();
	if cli.quiet {
		unimplemented!();
	}
	if !cli.config_path.exists() {
		println!("{} configuration file not found, generating default...", "notice:".cyan().bold());
		if let Some(parent) = cli.config_path.parent() {
			fs::create_dir_all(parent)?;
		}
		fs::write(&cli.config_path, default_configs::get())?;
		println!(
"Configuration saved in {}
Create backup using default configuration? [{}/{}]",
			cli.config_path.to_string_lossy().cyan().bold(),
			"y".cyan().bold(),
			"N".cyan().bold()
		);
		let mut choice = [0];
		io::stdin().read_exact(&mut choice)?;
		if choice[0].to_ascii_lowercase() != b'y' {
			process::exit(0);
		}
	}
	print!("{} configuration...\r", "Loading".cyan().bold());
	io::stdout().flush()?;
	let config: Table = toml::from_str(&fs::read_to_string(cli.config_path)?)?;
	CONFIG.set(Config {
		paths: parse_config_field!(config.backup.paths -> map!(
			"paths must be strings",
			value.as_str() -> |s| Ok(PathBuf::from_str(s).unwrap_or_else(handle_error))
		)),
		exclude: parse_config_field!(config.backup.exclude -> map!(
			"excluded patterns must be strings",
			value.as_str() -> |s| Ok(Regex::new(s).unwrap_or_else(handle_error))
		)),
		exclude_tags: parse_config_field!(config.backup.exclude_tags -> map!(
			"excluded tags must be arrays of arrays containing the path and the mode (both strings)",
			value.as_array() -> parse_excluded_tag
		)),
		name: Regex::new(r"%(![a-z]+)?([^% ]*)?")?.replace_all(
			&parse_config_field!(config.backup.name -> String),
			parse_name_capture
		).into_owned(),
		level: parse_config_field!(config.xz.level -> u32),
		memlimit: parse_config_field!(config.xz.memlimit -> u64),
		threads: parse_config_field!(config.xz.threads -> u32),
		block_size: parse_config_field!(config.xz.block_size -> u64),
	}).unwrap();
	println!("{} loading configuration!", "Finished".green().bold());
	std::process::exit(1);
}
