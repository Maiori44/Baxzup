use error::ResultExt;
use std::panic;

mod static_ptr;
mod error;
mod config;
mod backup;

macro_rules! input {
	($question:expr => { $($char:literal => $code:expr,)+ _ => $default:expr, }) => {{
		eprintln!("{}", $question);
		let mut choice = String::new();
		io::stdin().read_line(&mut choice).unwrap_or_exit();
		match choice.trim_start().as_bytes().first() {
			Some(byte) => match byte.to_ascii_lowercase() {
				$($char => $code,)+
				_ => $default,
			},
			None => $default,
		}
	}};
}

pub(crate) use input;

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
