use xz2::{read::XzEncoder, stream::MtStreamBuilder};
use crate::{config::Config, error::ResultExt};
use self::bars::BarsHandler;
use std::{io, fs::File};
use colored::Colorize;

mod bars;
mod tar;

pub fn init(config: Config) -> io::Result<()> {
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
	let bars_handler = BarsHandler::new(config.progress_bars, &compressor);
	let mut output_file = File::options().read(true).write(true).create_new(true).open(&config.name)?;
	let tar_thread = tar::spawn_thread(writer, config, &bars_handler);
	io::copy(&mut compressor, &mut output_file)?;
	if bars_handler.enabled {
		bars_handler.xz_bar.finish_with_message("Compressed ".green().bold().to_string());
		if !bars_handler.tar_bar.is_finished() {
			bars_handler.tar_bar.abandon_with_message("Archived?".yellow().bold().to_string());
		}
		bars_handler.end();
	}
	tar_thread.join().unwrap();
	Ok(())
}
