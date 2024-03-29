use fs_id::GetID;
use xz2::{read::XzEncoder, stream::MtStreamBuilder};
use crate::{backup::tar::SUBARCHIVE_VALUES, config::{assert_config, config}, error::ResultExt, input};
use self::bars::BarsHandler;
use std::{fs::{self, File, Metadata}, io::{self, Read}, path::Path, process, sync::OnceLock, thread};
use os_pipe::PipeReader;
use colored::Colorize;

pub mod bars;
mod tar;

struct ReaderObserver<R: Read>(R);

impl<R: Read> Read for ReaderObserver<R> {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		let read = self.0.read(buf)?;
		if read > 0 {
			println!(
				"Writing {} compressed bytes",
				read.to_string().cyan().bold()
			);
		}
		Ok(read)
	}
}

pub trait BorrowCompressor : Read {
	fn borrow_compressor(&mut self) -> &mut XzEncoder<PipeReader>;
}

impl BorrowCompressor for XzEncoder<PipeReader> {
	fn borrow_compressor(&mut self) -> &mut XzEncoder<PipeReader> {
		self
	}
}

impl BorrowCompressor for ReaderObserver<XzEncoder<PipeReader>> {
	fn borrow_compressor(&mut self) -> &mut XzEncoder<PipeReader> {
		&mut self.0
	}
}

pub fn metadata(path: impl AsRef<Path>) -> io::Result<Metadata> {
	if *config!(follow_symlinks) {
		path.as_ref().metadata()
	} else {
		path.as_ref().symlink_metadata()
	}
}

fn compress<T>(
	reader: PipeReader,
	f: impl FnOnce(&mut dyn BorrowCompressor) -> io::Result<T>,
) -> io::Result<()> {
	let config = config!();
	let compressor = XzEncoder::new_stream(
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
		// SAFETY: Only one thread has access to COMPRESSOR
		f(unsafe {
			let prev = COMPRESSOR.take();
			COMPRESSOR.set(compressor).unwrap_unchecked();
			let compressor = COMPRESSOR.get_mut().unwrap_unchecked();
			BarsHandler::set_ticker(compressor);
			drop(prev);
			compressor
		})?;
	} else {
		f(&mut ReaderObserver(compressor))?;
	}
	Ok(())
}

pub fn init() -> io::Result<()> {
	let config = config!();
	assert_config!(
		config.level > 9,
		"`{}` cannot exceed 9",
		"xz.level".yellow().bold()
	);
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
	if config.use_multiple_subarchives {
		let tar_thread = tar::spawn_thread(output_file, output_file_id);
		loop {
			thread::park();
			// SAFETY: The tar thread will drop the values only after this thread unparks it.
			let subarchive_values = unsafe {
				if SUBARCHIVE_VALUES.is_null() {
					break tar_thread;
				} else {
					SUBARCHIVE_VALUES.deref()
				}
			};
			compress(
				subarchive_values.reader.try_clone()?,
				|compressor| {
					unsafe {
						(subarchive_values.f)(
							compressor,
							&*subarchive_values.dir_path,
							&*subarchive_values.name_start,
							&mut *subarchive_values.builder,
						)
					}
				}
			)?;
			tar_thread.thread().unpark();
		}
	} else {
		let (reader, writer) = os_pipe::pipe()?;
		let tar_thread = tar::spawn_thread(writer, output_file_id);
		compress(reader, |compressor| io::copy(compressor, &mut output_file))?;
		tar_thread
	}.join().unwrap();
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
