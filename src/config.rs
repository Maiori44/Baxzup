use std::{
	path::PathBuf,
	process,
	io,
	fs,
};
use clap::{
	builder::{Styles, styling::{AnsiColor, Effects}},
	crate_description,
	crate_authors,
	crate_name,
	Parser,
};
use colored::Colorize;
use dirs::config_dir;
use crate::default_configs;

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

pub fn init() -> io::Result<()> {
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
Modify the default settings to your liking and then re-run the command to start the backup.",
			cli.config_path.to_string_lossy().cyan().bold()
		);
		process::exit(-1);
	}
	std::process::exit(1);
}
