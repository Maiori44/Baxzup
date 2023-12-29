use colored::Colorize;
use std::{
	process,
	fmt::Display,
};

mod default_configs;
mod config;

pub fn handle_error<T>(e: impl Display) -> T {
	eprintln!("{} {}", "error:".red().bold(), e.to_string());
	process::exit(-1);
}

fn main() {
	#[cfg(windows)]
	colored::control::set_virtual_terminal(true);
	config::init().unwrap_or_else(handle_error);
}
