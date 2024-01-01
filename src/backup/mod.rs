use indicatif::ProgressBar;
use xz2::{read::XzEncoder, stream::MtStreamBuilder};
use std::{io::{self, Write}, fs::File};
use crate::{config::Config, error::ResultExt};

mod tar;


pub struct WriterObserver<W: Write + Send + 'static> {
	writer: W,
	bar: ProgressBar,
	f: fn(&ProgressBar, u64),
}

impl<W: Write + Send + 'static> Write for WriterObserver<W> {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		let wrote = self.writer.write(buf)?;
		(self.f)(&self.bar, wrote as u64);
		Ok(wrote)
	}

	fn flush(&mut self) -> io::Result<()> {
		self.writer.flush()
	}
}

pub fn init(config: Config) -> io::Result<()> {
	let bar = ProgressBar::new(0);
	let (reader, writer) = os_pipe::pipe()?;
	let stream = MtStreamBuilder::new()
		.preset(config.level)
		.threads(config.threads)
		.block_size(config.block_size)
		.encoder()
		.to_io_result()?;
	let mut compressor = XzEncoder::new_stream(reader, stream);
	let output_file = File::options().read(true).write(true).create_new(true).open(&config.name)?;
	let tar_thread = tar::spawn_thread(writer, config, bar.clone());
	io::copy(&mut compressor, &mut WriterObserver { writer: output_file, bar, f: ProgressBar::inc })?;
	tar_thread.join().unwrap()
}
