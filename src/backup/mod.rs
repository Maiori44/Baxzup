use xz2::{read::XzEncoder, stream::MtStreamBuilder};
use crate::{config::config, error::ResultExt};
use self::bars::BarsHandler;
use std::{io, fs::File};
use colored::Colorize;

pub mod bars;
mod tar;

pub fn init() -> io::Result<()> {
	let config = config!();
	let (reader, writer) = os_pipe::pipe()?;
	let mut compressor = XzEncoder::new_stream(
		reader,
		MtStreamBuilder::new()
			.preset(config.level)
			.threads(config.threads)
			.block_size(config.block_size)
			.encoder()
			.to_io_result()?
	);
	BarsHandler::init(&compressor);
	let mut output_file = File::options().read(true).write(true).create_new(true).open(&config.name)?;
	#[cfg(target_os = "windows")]
	{
		use fs4::FileExt;
		output_file.try_lock_exclusive()?;
	}
	let tar_thread = tar::spawn_thread(writer);
	io::copy(&mut compressor, &mut output_file)?;
	BarsHandler::end(|bars_handler| {
		bars_handler.xz_bar.finish_with_message("Compressed ".green().bold().to_string());
		if !bars_handler.tar_bar.is_finished() {
			bars_handler.tar_bar.abandon_with_message("Archived?".yellow().bold().to_string());
		}
	});
	tar_thread.join().unwrap();
	Ok(())
}
