use std::{
	collections::HashMap,
	error::Error,
	ffi::OsString,
	fmt::Debug,
	hint::unreachable_unchecked,
	path::PathBuf,
	process,
	str::FromStr,
	sync::Mutex,
	env,
	fs,
	io,
};
use chrono::{Local, format::{DelayedFormat, Item, StrftimeItems}};
use clap::{
	builder::{Styles, styling::{AnsiColor, Effects}},
	crate_description,
	crate_authors,
	crate_name,
	ValueEnum,
	Parser,
};
use colored::Colorize;
use dirs::config_dir;
use regex::{bytes, Regex, Captures};
use sysinfo::{System, User, RefreshKind, ProcessRefreshKind, Users};
use toml::{value::Array, Table, Value};
use crate::{
	backup::bars::{spinner_chars, PROGRESS_BAR},
	error::{self, ResultExt},
	static_ptr::StaticPointer,
	input,
};

mod default;

macro_rules! map {
	($name:expr, $err:literal, value.$as:ident() -> $f:expr) => {
		$name
			.$as()
			.map_or_else(|| Err($err), $f)
			.unwrap_or_exit()
	};
	($name:expr, value$(.$as:ident())? -> $f:expr) => {
		$f($name$(.$as())?).unwrap_or_exit()
	};
}

macro_rules! config {
	() => {
		//SAFETY: The configuration will always be initialized by the time this macro is used.
		unsafe {
			crate::config::CONFIG.deref()
		}
	};
	($field:ident) => {
		&config!().$field
	};
	($($field:ident),+) => {{
		let config = config!();
		($(&config.$field),+)
	}};
}

pub(crate) use config;

macro_rules! assert_config {
	($test:expr, $($arg:tt)*) => {
		if $test {
			return Err(io::Error::other(format!($($arg)*)));
		}
	}
}

pub(crate) use assert_config;

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
	about = format!(
		"{}\nMade by {}\nhttps://github.com/Maiori44/Baxzup",
		crate_description!(),
		crate_authors!()
	),
	long_about = None
)]
struct Cli {
	/// Path to the configuration file
	#[arg(
		short,
		long,
		value_name = "PATH",
		default_value(match config_dir() {
			Some(mut dir) => {
				dir.push(crate_name!());
				dir.push(crate_name!());
				dir.set_extension("toml");
				dir.into_os_string()
			},
			None => {
				let mut str = OsString::from(crate_name!());
				str.push(".toml");
				str
			},
		})
	)]
	config_path: PathBuf,

	/// Use the default configuration rather than reading the configuration file
	#[arg(short = 'd', long, conflicts_with = "config_path")]
	default_config: bool,

	/// Don't print non-important messages
	#[arg(short, long)]
	quiet: bool,

	/// Colorize the output
	#[clap(long, value_enum, ignore_case(true), default_value = "auto", value_name = "WHEN")]
	color: ColorMode,

	/// Paths to the directories/files to add to the backup [default: use configuration]
	#[arg(short, long, value_delimiter = ',')]
	paths: Option<Vec<PathBuf>>,

	/// Add more paths to the list of paths to backup
	#[arg(short = 'P', long, value_delimiter = ',', value_name = "PATHS")]
	add_paths: Vec<PathBuf>,

	/// List of patterns to exclude [default: use configuration]
	#[arg(short, long, value_delimiter = ',', value_name = "PATTERNS")]
	exclude: Option<Vec<String>>,

	/// Add more patterns to the list of excluded patterns
	#[arg(short = 'E', long, value_delimiter = ',', value_name = "PATTERNS")]
	add_exclude: Vec<String>,

	/// Ignore the excluded tags defined in the configuration file
	#[arg(long)]
	allow_tags: bool,

	/// Add an excluded tag with the keep-tag mode (keep folder with only the tag file inside)
	#[arg(long, value_delimiter = ',', value_name = "TAGS")]
	exclude_tags: Vec<OsString>,

	/// Add an excluded tag with the keep-dir mode (keep folder without any file inside)
	#[arg(long, value_delimiter = ',', value_name = "TAGS")]
	exclude_tags_under: Vec<OsString>,

	/// Add an excluded tag with the keep-none mode (don't keep anything)
	#[arg(long, value_delimiter = ',', value_name = "TAGS")]
	exclude_tags_all: Vec<OsString>,

	/// Archive what symlinks link to rather than the symlink itself, may get stuck in a loop
	/// [default: use configuration]
	#[arg(short = 's', long, value_name = "FOLLOW", default_missing_value = "true", num_args = 0..=1)]
	follow_symlinks: Option<bool>,

	/// Skip files that failed to be read rather than asking the user [default: use configuration]
	#[arg(short, long, value_name = "IGNORE", default_missing_value = "true", num_args = 0..=1)]
	ignore_unreadable_files: Option<bool>,

	/// Replace any already existing file with the same name as the backup [default: use configuration]
	#[arg(short, long, value_name = "FORCE", default_missing_value = "true", num_args = 0..=1)]
	force_overwrite: Option<bool>,

	/// Create an uncompressed archive containing compressed subarchives [default: use configuration]
	#[arg(short = 'm', long, value_name = "ENABLE", default_missing_value = "true", num_args = 0..=1)]
	use_multiple_subarchives: Option<bool>,

	/// Name (or path) of the backup file [default: use configuration]
	#[arg(short, long)]
	name: Option<String>,

	/// Show 2 progress bars displaying how much was archived and compressed [default: use configuration]
	#[arg(long, value_name = "ENABLE", default_missing_value = "true", num_args = 0..=1)]
	progress_bars: Option<bool>,

	/// The colored to be used for the "Archiving" progress bar [default: use configuration]
	#[arg(long, value_name = "COLOR")]
	tar_bar_color: Option<String>,

	/// The colored to be used for the "Compressing" progress bar [default: use configuration]
	#[arg(long, value_name = "COLOR")]
	xz_bar_color: Option<String>,

	/// Compression level for XZ (0-9) [default: use configuration]
	#[arg(short, long)]
	level: Option<u32>,

	/// Amount of threads used by XZ [default: use configuration]
	#[arg(short, long)]
	threads: Option<u32>,

	/// Size of each uncompressed block used by XZ in bytes [default: use configuration]
	#[arg(short, long)]
	block_size: Option<u64>,

	/// Update any outdated configuration automatically instead of asking
	#[arg(short, long)]
	auto_update_config: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ColorMode {
	Auto,
	Always,
	Never
}

trait ToOption<T> {
	fn to_option(self) -> Option<T>;
}

impl ToOption<[(); 0]> for bool {
	fn to_option(self) -> Option<[(); 0]> {
		self.then_some([])
	}
}

impl<T> ToOption<T> for Option<T> {
	fn to_option(self) -> Option<T> {
		self
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagKeepMode {
	/// Keep the tagged folder with only the tag inside.
	Tag,

	/// Keep the tagged folder with no file inside.
	Dir,

	/// Don't keep the tagged folder at all.
	None,
}

#[derive(Debug)]
pub struct Config {
	pub paths: Vec<PathBuf>,
	pub exclude: Vec<bytes::Regex>,
	pub exclude_tags: HashMap<OsString, TagKeepMode>,
	pub follow_symlinks: bool,
	pub ignore_unreadable_files: Mutex<bool>,
	pub force_overwrite: bool,
	pub use_multiple_subarchives: bool,
	pub name: String,
	pub progress_bars: bool,
	pub spinner_chars: String,
	pub progress_chars: String,
	pub tar_bar_color: String,
	pub xz_bar_color: String,
	pub level: u32,
	pub threads: u32,
	pub block_size: u64,
}

pub static mut CONFIG: StaticPointer<Config> = StaticPointer::null();

fn parse_excluded_tag((name, mode): (&String, &Value)) -> Result<(OsString, TagKeepMode), String> {
	Ok((OsString::from(name), map!(
		mode,
		"excluded tag modes must be strings",
		value.as_str() -> |s| Ok(match s.to_ascii_lowercase().as_str() {
			"keep-tag" | "keep tag" | "keep_tag" | "keeptag" => TagKeepMode::Tag,
			"keep-dir" | "keep dir" | "keep_dir" | "keepdir" => TagKeepMode::Dir,
			"keep-none" | "keep none" | "keep_none" | "keepnone" => TagKeepMode::None,
			_ => return Err("unknown tag mode"),
		})
	)))
}

fn get_from_user<T>(f: impl FnOnce(&User) -> T) -> Option<T> {
	let users = Users::new_with_refreshed_list();
	let system = System::new_with_specifics(
		RefreshKind::new()
			.with_processes(ProcessRefreshKind::new().with_user(sysinfo::UpdateKind::Always))
	);
	users.get_user_by_id(
		system.process(sysinfo::get_current_pid().unwrap())
			.unwrap()
			.user_id()?
	).map(f)
}

fn unknown() -> String {
	String::from("unknown")
}

fn parse_excluded_pattern(s: &str) -> Result<bytes::Regex, &str> {
	Ok(bytes::Regex::new(&match Regex::new(r"^\?/(.*)/([imsUx]+)?$").unwrap().captures(s) {
		Some(captures) => [
			captures.get(2).map_or_else(String::new, |m| format!("(?{})", m.as_str())),
			captures.get(1).unwrap().as_str().to_string()
		].into_iter().collect::<String>(),
		None => Regex::new(r"[-\[\]{}()*+?.,\\^$|#]")
			.unwrap()
			.replace_all(s.strip_suffix(
				#[cfg(windows)]
				'\\',
				#[cfg(unix)]
				'/'
			).unwrap_or(s), "\\$0")
			.to_string() + "$",
	}).unwrap_or_exit())
}

fn format_items(items: Vec<Item>) -> String {
	let date = Local::now();
	DelayedFormat::new_with_offset(
		Some(date.date_naive()),
		Some(date.time()),
		date.offset(),
		items.iter()
	).to_string()
}

fn parse_name_capture(caps: &Captures) -> String {
	if let Some(group) = caps.get(1) {
		let result = match group.as_str() {
			"!hostname" => Some(System::host_name().unwrap_or_else(unknown)),
			"!systemname" => Some(System::name().unwrap_or_else(unknown)),
			"!systemid" => Some(System::distribution_id()),
			"!username" => Some(get_from_user(|user| user.name().to_string()).unwrap_or_else(unknown)),
			#[cfg(windows)]
			"!groupname" => Some(unknown()),
			#[cfg(unix)]
			"!groupname" => Some(match get_from_user(|user| {
				user.groups().into_iter().find(|x| *x.id() == user.group_id())
			}) {
				Some(group) => {
					group.map_or_else(unknown, |group| String::from(group.name()))
				},
				None => unknown()
			}),
			_ => None,
		};
		if let Some(result) = result {
			match caps.get(2) {
				Some(end) => result + end.as_str(),
				None => result
			}
		} else {
			let invalid = String::from("%") + group.as_str();
			error::handler(format!("unknown specifier `{}`", invalid.yellow().bold()))
		}
	} else {
		let captured = caps.get(0).unwrap().as_str();
		let mut iter = StrftimeItems::new(captured);
		let mut items = Vec::new();
		while let Some(item) = iter.next() {
			if item == Item::Error {
				items.clear();
				for item in iter {
					if item != Item::Error {
						items.push(item);
					}
				}
				error::handler(format!(
					"unknown specifier `{}`",
					captured.strip_suffix(&format_items(items)).unwrap_or(captured).yellow().bold()
				));
			} else {
				items.push(item);
			}
		}
		format_items(items)
	}
}

pub fn init() -> Result<(), Box<dyn Error>> {
	let cli = Cli::parse();
	match cli.color {
		ColorMode::Auto => env::set_var("CLICOLOR", "1"),
		ColorMode::Always => env::set_var("CLICOLOR_FORCE", "1"),
		ColorMode::Never => {
			env::set_var("CLICOLOR_FORCE", "0");
			env::set_var("CLICOLOR", "0");
		}
	}
	if cli.quiet {
		//force SHOULD_COLORIZE to be created before stdout is silenced,
		//to avoid it mistakenly disabling colors
		colored::control::SHOULD_COLORIZE.should_colorize();
		Box::leak(Box::new(shh::stdout()?));
	}
	let config_path_str = if cli.default_config {
		"--default-config".cyan().bold()
	} else {
		cli.config_path.to_string_lossy().cyan().bold()
	};
	if !(cli.config_path.exists() || cli.default_config) {
		println!("{} configuration file not found, generating default...", "notice:".cyan().bold());
		if let Some(parent) = cli.config_path.parent() {
			fs::create_dir_all(parent)?;
		}
		fs::write(&cli.config_path, default::get().to_string())?;
		input!(format!(
"Default configuration saved in `{config_path_str}`
Create backup using default configuration? [{}/{}]",
			"y".cyan().bold(),
			"N".cyan().bold()
		) => {
			b'y' => {},
			_ => process::exit(0),
		});
	}
	println!("{} configuration... (`{config_path_str}`)", "Loading".cyan().bold());
	let mut config = if cli.default_config {
		default::get()
	} else {
		toml::from_str(&fs::read_to_string(&cli.config_path)?)?
	};
	
	macro_rules! parse_config_field {
		(config.$i1:ident.$i2:ident?) => {{
			config
				.get(stringify!($i1))
				.ok_or_else(|| format!(
					"Missing table `{}` in configuration file!",
					stringify!($i1).cyan().bold()
				))?
				.get(stringify!($i2))
		}};
		(config.$i1:ident.$i2:ident) => {{
			parse_config_field!(config.$i1.$i2?)
				.ok_or_else(|| format!(
					"Could not find field `{}` in configuration file!",
					format!("{}.{}", stringify!($i1), stringify!($i2)).cyan().bold()
				))?
		}};
		(config.$i1:ident.$i2:ident $([default: $default:expr])?
		-> map!($type:ty, $($err:literal,)? value$(.$as:ident())? -> $f:expr)) => {
			parse_config_field!(config.$i1.$i2 $([default: $default])? -> $type)
				.iter()
				.map(|value| {
					map!(value, $($err,)? value$(.$as())? -> $f)
				})
				.collect()
		};
		(cli.$cli:ident || config.$i1:ident.$i2:ident $($rest:tt)*) => {
			match cli.$cli.to_option() {
				Some(field) => field,
				None => parse_config_field!(config.$i1.$i2 $($rest)*)
			}
		};
		(cli.$cli:ident -> map!($type:ty, $f:expr) || config.$i1:ident.$i2:ident $($rest:tt)*) => {
			match ToOption::<$type>::to_option(cli.$cli) {
				Some(field) => field.iter().map(|value| map!(value, value -> $f)).collect(),
				None => parse_config_field!(config.$i1.$i2 $($rest)*)
			}
		};
		(cli.$cli:ident -> map!($f:expr) || config.$i1:ident.$i2:ident $($rest:tt)*) => {
			parse_config_field!(cli.$cli -> map!([(); 0], $f) || config.$i1.$i2 $($rest)*)
		};
		(config.$i1:ident.$i2:ident -> $type:ty) => {
			parse_config_field!(config.$i1.$i2)
				.clone()
				.try_into::<$type>()
				.map_err(|e| format!(
					"failed to parse field `{}`\n{e}",
					format!("{}.{}", stringify!($i1), stringify!($i2)).cyan().bold()
				))?
		};
		(config.$i1:ident.$i2:ident [default: $default:expr] -> $type:ty) => {
			match parse_config_field!(config.$i1.$i2?) {
				Some(field) => field.clone().try_into::<$type>().map_err(|e| format!(
					"failed to parse field `{}`\n{e}",
					format!("{}.{}", stringify!($i1), stringify!($i2)).cyan().bold()
				))?,
				None => $default,
			}
		};
	}

	if parse_config_field!(config.backup.exclude_tags?).is_some_and(|value| value.is_array()) {
		default::update(
			cli.auto_update_config,
			format!(
				"{} outdated type (`{}`) found for field `{}` (replaced by `{}`)",
				"warning:".yellow().bold(),
				"[[String, String], ...]".yellow().bold(),
				"backup.exclude_tags".cyan().bold(),
				"Table<String>".cyan().bold(),
			),
			&mut config,
			|update, config| {
				let tags_value = config["backup"]
					.as_table_mut()
					.unwrap()
					.remove("exclude_tags")
					.unwrap();
				let tags = tags_value.as_array().unwrap();
				update(config);
				let mut table = Table::with_capacity(tags.len());
				for tag in tags {
					if let Some([Value::String(name), mode]) = tag.as_array().map(Vec::as_slice) {
						table.insert(name.to_owned(), mode.clone());
					}
				}
				config["backup"]["exclude_tags"] = Value::Table(table);
				fs::write(&cli.config_path, config.to_string())
			}
		)?;
	}
	if parse_config_field!(config.backup.progress_bars?).is_some() {
		default::update(
			cli.auto_update_config,
			format!(
				"{} outdated field `{}` found (replaced by `{}`)",
				"warning:".yellow().bold(),
				"backup.progress_bars".yellow().bold(),
				"progress_bars.enable".cyan().bold(),
			),
			&mut config,
			|update, config| {
				let value = config["backup"].as_table_mut().unwrap().remove("progress_bars").unwrap();
				update(config);
				config["progress_bars"]["enable"] = value;
				fs::write(&cli.config_path, config.to_string())
			}
		)?;
	}
	let mut config = Box::new(Config {
		paths: parse_config_field!(cli.paths || config.backup.paths -> map!(
			Array,
			"paths must be strings",
			value.as_str() -> |s| Ok(PathBuf::from_str(s).unwrap_or_exit())
		)),
		exclude: parse_config_field!(
			cli.exclude -> map!(
				Vec<String>,
				parse_excluded_pattern
			)
			|| config.backup.exclude -> map!(
				Array,
				"excluded patterns must be strings",
				value.as_str() -> parse_excluded_pattern
			)
		),
		#[allow(clippy::redundant_closure_call)]
		exclude_tags: parse_config_field!(
			cli.allow_tags -> map!(|_: &_| -> Result<(OsString, TagKeepMode), &str> {
				unsafe { unreachable_unchecked() }
			})
			|| config.backup.exclude_tags -> map!(
				Table,
				value -> parse_excluded_tag
			)
		),
		follow_symlinks: parse_config_field!(
			cli.follow_symlinks || config.backup.follow_symlinks [default: false] -> bool
		),
		ignore_unreadable_files: Mutex::new(parse_config_field!(
			cli.ignore_unreadable_files
			|| config.backup.ignore_unreadable_files [default: false] -> bool
		)),
		force_overwrite: parse_config_field!(
			cli.force_overwrite
			|| config.backup.force_overwrite [default: false] -> bool
		),
		use_multiple_subarchives: parse_config_field!(
			cli.use_multiple_subarchives
			|| config.backup.use_multiple_subarchives [default: false] -> bool
		),
		name: Regex::new(r"%(![a-z]+)?([^% ]*)?")?.replace_all(
			&parse_config_field!(cli.name || config.backup.name -> String),
			parse_name_capture
		).into_owned(),
		progress_bars: if cli.quiet {
			false
		} else {
			parse_config_field!(cli.progress_bars || config.progress_bars.enable -> bool)
		},
		spinner_chars: parse_config_field!(
			config.progress_bars.spinner_chars [default: spinner_chars().to_string()] -> String
		),
		progress_chars: parse_config_field!(
			config.progress_bars.progress_chars [default: String::from(PROGRESS_BAR)] -> String
		),
		tar_bar_color: parse_config_field!(
			cli.tar_bar_color
			|| config.progress_bars.tar_bar_color [default: String::from("yellow")] -> String
		),
		xz_bar_color: parse_config_field!(
			cli.xz_bar_color
			|| config.progress_bars.xz_bar_color [default: String::from("magenta")] -> String
		),
		level: parse_config_field!(cli.level || config.xz.level -> u32),
		threads: parse_config_field!(cli.threads || config.xz.threads -> u32),
		block_size: parse_config_field!(cli.block_size || config.xz.block_size [default: 0] -> u64),
	});
	config.paths.extend(cli.add_paths);
	config.exclude.extend(
		cli.add_exclude
			.into_iter()
			.map(|value| map!(value, value.as_str() -> parse_excluded_pattern))
	);
	config.exclude_tags.extend(
		cli.exclude_tags
			.into_iter()
			.map(|tag| (tag, TagKeepMode::Tag))
			.chain(
				cli.exclude_tags_under
					.into_iter()
					.map(|tag| (tag, TagKeepMode::Dir))
			)
			.chain(
				cli.exclude_tags_all
					.into_iter()
					.map(|tag| (tag, TagKeepMode::None))
			)
	);
	// SAFETY: There is only one thread running for now
	unsafe { CONFIG.set(Box::leak(config)) }
	println!(
		"{}{} configuration! (`{config_path_str}`)",
		if *config!(progress_bars) {
			"\x1b[2J\x1b[H"
		} else {
			"\x1b[A\x1b[K"
		},
		"Loaded".green().bold()
	);
	Ok(())
}
