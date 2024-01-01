use xz2::{bufread::XzEncoder, stream::MtStreamBuilder};
use std::{io::{self, BufReader, Write}, fs::File};
use crate::{config::Config, error::ResultExt};

mod tar;


pub struct WriterObserver<W: Write + Send + 'static> {
	writer: W,
}

impl<W: Write + Send + 'static> Write for WriterObserver<W> {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		let wrote = self.writer.write(buf)?;
		Ok(wrote)
	}

	fn flush(&mut self) -> io::Result<()> {
		self.writer.flush()
	}
}

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
