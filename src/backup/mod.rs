use fs_id::GetID;
use xz2::{read::XzEncoder, stream::MtStreamBuilder};
use crate::{config::{config, assert_config}, error::ResultExt, input};
use self::bars::BarsHandler;
use std::{fs::{self, File}, io::{self, Write}, path::Path, sync::OnceLock, process, thread};
use os_pipe::PipeReader;
use colored::Colorize;

pub mod bars;
mod tar;

struct WriterObserver<W: Write> (W);

impl<W: Write> Write for WriterObserver<W> {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		println!(
			"Writing {} compressed bytes",
			buf.len().to_string().cyan().bold()
		);
		self.0.write(buf)
	}

	fn flush(&mut self) -> io::Result<()> {
		self.0.flush()
	}
}

pub fn compress(output_file: &mut File, reader: PipeReader) -> io::Result<()> {
	let config = config!();
	assert_config!(
		config.level > 9,
		"`{}` cannot exceed 9",
		"xz.level".yellow().bold()
	);
	let mut compressor = XzEncoder::new_stream(
		reader,
		MtStreamBuilder::new()
			.preset(config.level)
			.threads(if config.threads == 0 {
				thread::available_parallelism()?.get() as u32
			} else {
				config.threads
			})
			.block_size(config.block_size)
			.encoder()
			.to_io_result()?
	);
	if config.progress_bars {
		static mut COMPRESSOR: OnceLock<XzEncoder<PipeReader>> = OnceLock::new();
		io::copy(
			// SAFETY: Only one thread has access to COMPRESSOR
			unsafe {
				let prev = COMPRESSOR.take();
				COMPRESSOR.set(compressor).unwrap_unchecked();
				let compressor = COMPRESSOR.get_mut().unwrap_unchecked();
				BarsHandler::set_ticker(compressor);
				drop(prev);
				compressor
			},
			output_file
		)?;
	} else {
		io::copy(&mut compressor, &mut WriterObserver(output_file))?;
	}
	Ok(())
}

pub fn init() -> io::Result<()> {
	let config = config!();
	let (reader, writer) = os_pipe::pipe()?;
	let path_name = Path::new(&config.name);
	let mut output_file = if config.force_overwrite {
		File::create(path_name)?
	} else {
		if path_name.exists() {
			input!(format!(
				"{} a file named `{}` already exists\nOverwrite? [{}/{}]",
				"warning:".yellow().bold(),
				config.name.cyan().bold(),
				"y".cyan().bold(),
				"N".cyan().bold()
			) => {
				b'y' => fs::remove_file(path_name)?,
				_ => process::exit(0),
			})
		}
		File::options()
			.read(true)
			.write(true)
			.create_new(true)
			.open(path_name)?
	};
	if let Some(parent) = path_name.parent() {
		fs::create_dir_all(parent)?;
	}
	let output_file_id = output_file.get_id()?;
	BarsHandler::init(output_file_id)?;
	let tar_thread = tar::spawn_thread(writer, output_file_id);
	if true {
		compress(&mut output_file, reader)?;
	} else {
		todo!()
	}
	BarsHandler::end(|bars_handler| {
		bars_handler.status_bar.inc(1);
		bars_handler.status_bar.finish_with_message(format!(
			"Finished creating `{}`!",
			config.name.cyan().bold()
		));
		bars_handler.xz_bar.finish_with_message("Compressed ".green().bold().to_string());
		if !bars_handler.tar_bar.is_finished() {
			bars_handler.tar_bar.abandon_with_message("Archived?".yellow().bold().to_string());
		}
	});
	if !config.progress_bars {
		println!(
			"Finished creating `{}`!",
			config.name.cyan().bold()
		);
	}
	tar_thread.join().unwrap();
	Ok(())
	/*let config = config!();
	let (reader, writer) = os_pipe::pipe()?;
	assert_config!(
		config.level > 9,
		"`{}` cannot exceed 9",
		"xz.level".yellow().bold()
	);
	let mut compressor = XzEncoder::new_stream(
		reader,
		MtStreamBuilder::new()
			.preset(config.level)
			.threads(if config.threads == 0 {
				thread::available_parallelism()?.get() as u32
			} else {
				config.threads
			})
			.block_size(config.block_size)
			.encoder()
			.to_io_result()?
	);
	let path_name = Path::new(&config.name);
	if let Some(parent) = path_name.parent() {
		fs::create_dir_all(parent)?;
	}
	let mut output_file = if config.force_overwrite {
		File::create(path_name)?
	} else {
		if path_name.exists() {
			input!(format!(
				"{} a file named `{}` already exists\nOverwrite? [{}/{}]",
				"warning:".yellow().bold(),
				config.name.cyan().bold(),
				"y".cyan().bold(),
				"N".cyan().bold()
			) => {
				b'y' => fs::remove_file(path_name)?,
				_ => process::exit(0),
			})
		}
		File::options()
			.read(true)
			.write(true)
			.create_new(true)
			.open(path_name)?
	};
	let output_file_id = output_file.get_id()?;
	BarsHandler::init(&compressor, output_file_id)?;
	let tar_thread = tar::spawn_thread(writer, output_file_id);
	if config.progress_bars {
		io::copy(&mut compressor, &mut output_file)
	} else {
		io::copy(&mut compressor, &mut WriterObserver(output_file))
	}?;
	BarsHandler::end(|bars_handler| {
		bars_handler.status_bar.inc(1);
		bars_handler.status_bar.finish_with_message(format!(
			"Finished creating `{}`!",
			config.name.cyan().bold()
		));
		bars_handler.xz_bar.finish_with_message("Compressed ".green().bold().to_string());
		if !bars_handler.tar_bar.is_finished() {
			bars_handler.tar_bar.abandon_with_message("Archived?".yellow().bold().to_string());
		}
	});
	if !config.progress_bars {
		println!(
			"Finished creating `{}`!",
			config.name.cyan().bold()
		);
	}
	tar_thread.join().unwrap();
	Ok(())*/
}
