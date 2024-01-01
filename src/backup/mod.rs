use xz2::{bufread::XzEncoder, stream::MtStreamBuilder};
use std::{io::{self, BufReader}, fs::File};
use crate::{config::config, error::ResultExt};

mod scanner;
mod tar;

pub fn init() -> io::Result<()> {
	let config = config!();	
	let (reader, writer) = os_pipe::pipe()?;
	let tar_thread = tar::spawn_thread(writer);
	let stream = MtStreamBuilder::new()
		.preset(config.level)
		.threads(config.threads)
		.block_size(config.block_size)
		.encoder()
		.to_io_result()?;
	let mut compressor = XzEncoder::new_stream(BufReader::new(reader), stream);
	let mut output_file = File::options().read(true).write(true).create_new(true).open(config!(name))?;
	io::copy(&mut compressor, &mut output_file)?;
	tar_thread.join().unwrap()
}
