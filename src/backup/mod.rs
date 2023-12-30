use std::{io::{self, Read, Write}, fs::File};
use xz2::{read::XzEncoder, stream::MtStreamBuilder};
use crate::{config::config, error::ResultExt};

mod scanner;
mod tar;

pub fn init() -> io::Result<()> {
	let (reader, writer) = os_pipe::pipe()?;
	let tar_thread = tar::spawn_thread(writer);
	let config = config!();
	let mut compressor = XzEncoder::new_stream(
		reader,
		MtStreamBuilder::new()
			.preset(config.level)
			.threads(config.threads)
			.block_size(config.block_size)
			.encoder()
			.to_io_result()?
	);
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