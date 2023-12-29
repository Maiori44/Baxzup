use std::{
	path::PathBuf,
	fs::File,
	process,
	io,
	fs,
};
use clap::{
	builder::{Styles, styling::{AnsiColor, Effects}},
	crate_description,
	crate_authors,
	Parser,
};
use colored::Colorize;
use toml::toml;

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
		default_value = if let Ok(home) = std::env::var("HOME") {
			home + "/.config/xz-backup/config.toml"
		} else {
			String::from("config.toml")
		}
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
		fs::write(&cli.config_path, toml!(
			[backup]
			paths = []
		).to_string())?;
		println!(
"Configuration saved in {}
Modify the default settings to your liking and then re-run the command to start the backup.",
			cli.config_path.to_string_lossy().cyan().bold()
		);
		process::exit(-1);
	}
	std::process::exit(1);
}
