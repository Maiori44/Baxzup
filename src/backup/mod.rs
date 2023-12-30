use std::{io::{self, Read, Write}, fs::File};
use xz2::read::XzEncoder;
use crate::config::config;

mod scanner;
mod tar;

pub fn init() -> io::Result<()> {
	let (reader, writer) = os_pipe::pipe()?;
	let tar_thread = tar::spawn_thread(writer);
	let mut compressor = XzEncoder::new(reader, 9); //TODO: apply config
	let mut output = File::options().read(true).write(true).create_new(true).open(config!(name))?;
	let mut buf = [0; 10240];
	loop {
		let read = compressor.read(&mut buf)?;
		if read == 0 {
			break tar_thread.join().unwrap();
		}
		output.write(&mut buf[..read])?;
	}
}