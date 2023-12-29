use colored::Colorize;
use std::{
	process,
	io,
};

mod default_configs;
mod config;

fn handle_error<T>(e: io::Error) -> T {
	eprintln!("{} {}", "error:".red().bold(), e.to_string().to_ascii_lowercase());
	process::exit(e.raw_os_error().unwrap_or(-2))
}

fn main() {
	#[cfg(windows)]
	colored::control::set_virtual_terminal(true);
	config::init().unwrap_or_else(handle_error);
}
