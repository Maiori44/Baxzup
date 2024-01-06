use error::ResultExt;
use std::panic;

mod error;
mod config;
mod backup;

#[cfg(feature = "pause")]
pub fn pause() {
	let mut buf = String::new();
    let _ = std::io::stdin().read_line(&mut buf);
}

fn main() {
	#[cfg(windows)]
	colored::control::set_virtual_terminal(true).unwrap();
	panic::set_hook(Box::new(error::panic_hook));
	config::init().unwrap_or_exit();
	backup::init().unwrap_or_exit();
	#[cfg(feature = "pause")]
	pause();
}
