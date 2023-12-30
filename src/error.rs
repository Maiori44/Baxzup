use std::{io, fmt::Display, process};
use colored::Colorize;

pub trait ResultExt<T> {
	fn unwrap_or_exit(self) -> T;

	fn to_io_result(self) -> io::Result<T>;
}

pub fn handler(e: impl Display) -> ! {
	eprintln!("{} {e}", "error:".red().bold());
	process::exit(-1)
}

impl<T, E: Display> ResultExt<T> for Result<T, E> {
	fn unwrap_or_exit(self) -> T {
		match self {
			Ok(t) => t,
			Err(e) => handler(e),
		}
	}

	fn to_io_result(self) -> io::Result<T> {
		match self {
			Ok(t) => Ok(t),
			Err(e) => Err(io::Error::other(e.to_string())),
		}
	}
}
