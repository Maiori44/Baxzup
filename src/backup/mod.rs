use xz2::{bufread::XzEncoder, stream::MtStreamBuilder};
use std::{io::{self, BufReader}, fs::File};
use crate::{config::Config, error::ResultExt};

mod tar;

pub fn init(config: Config) -> io::Result<()> {
	let (reader, writer) = os_pipe::pipe()?;
	let stream = MtStreamBuilder::new()
		.preset(config.level)
		.threads(config.threads)
		.block_size(config.block_size)
		.encoder()
		.to_io_result()?;
	let mut compressor = XzEncoder::new_stream(BufReader::new(reader), stream);
	let mut output_file = File::options().read(true).write(true).create_new(true).open(&config.name)?;
	let tar_thread = tar::spawn_thread(writer, config);
	io::copy(&mut compressor, &mut output_file)?;
	tar_thread.join().unwrap()
}
