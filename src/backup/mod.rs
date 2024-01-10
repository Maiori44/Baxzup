use xz2::{read::XzEncoder, stream::MtStreamBuilder};
use crate::{config::{config, Config}, error::ResultExt, input};
use self::bars::BarsHandler;
use std::{io, fs::{self, File}, path::Path, process};
use colored::Colorize;

pub mod bars;
mod tar;

#[cfg(target_os = "windows")]
pub type OutputFileID = ();

#[cfg(target_os = "windows")]
pub fn get_output_file_id(_: &Config) -> OutputFileID {}

#[cfg(not(target_os = "windows"))]
pub type OutputFileID = (u64, u64);

#[cfg(not(target_os = "windows"))]
pub fn get_output_file_id(config: &Config) -> OutputFileID {
    use std::os::unix::fs::MetadataExt;

	let meta = fs::metadata(&config.name).unwrap_or_exit();
	(meta.dev(), meta.ino())
}

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
	let mut output_file = if config.force_overwrite {
		File::create(&config.name)?
	} else {
		if AsRef::<Path>::as_ref(&config.name).exists() {
			input!(format!(
				"{} a file named '{}' already exists\nOverwrite? [{}/{}]",
				"warning:".yellow().bold(),
				config.name.cyan().bold(),
				"y".cyan().bold(),
				"N".cyan().bold()
			) => {
				b'y' => fs::remove_file(&config.name)?,
				_ => process::exit(0),
			})
		}
		File::options()
			.read(true)
			.write(true)
			.create_new(true)
			.open(&config.name)?
	};
	BarsHandler::init(&compressor);
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
