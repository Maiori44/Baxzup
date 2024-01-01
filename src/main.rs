use error::ResultExt;

mod error;
mod config;
mod backup;

fn main() {
	#[cfg(windows)]
	colored::control::set_virtual_terminal(true);
	let config = config::init().unwrap_or_exit();
	backup::init(config).unwrap_or_exit();
}
