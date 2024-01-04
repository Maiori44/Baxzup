use error::ResultExt;
use std::panic;

mod error;
mod config;
mod backup;

fn main() {
	#[cfg(windows)]
	colored::control::set_virtual_terminal(true).unwrap();
	panic::set_hook(Box::new(error::panic_hook));
	let config = config::init().unwrap_or_exit();
	backup::init(config).unwrap_or_exit();
}
