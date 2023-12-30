use colored::Colorize;
use std::{
	process,
	fmt::Display,
};

mod config;

pub fn handle_error<T>(e: impl Display) -> T {
	eprintln!("{} {e}", "error:".red().bold());
	process::exit(-1);
}

fn main() {
	#[cfg(windows)]
	colored::control::set_virtual_terminal(true);
	config::init().unwrap_or_else(handle_error);
}
