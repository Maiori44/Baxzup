use fs_id::GetID;
use xz2::{read::XzEncoder, stream::MtStreamBuilder};
use crate::{config::{config, assert_config}, error::ResultExt, input};
use self::bars::BarsHandler;
use std::{io::{self, Write}, fs::{self, File}, path::Path, process};
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

pub fn init() -> io::Result<()> {
	let config = config!();
	let (reader, writer) = os_pipe::pipe()?;
	assert_config!(
		config.level > 9,
		"`{}` cannot exceed 9",
		"xz.level".yellow().bold()
	);
	assert_config!(
		config.threads < 1,
		"`{}` must be at least 1",
		"xz.threads".yellow().bold()
	);
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
				"{} a file named `{}` already exists\nOverwrite? [{}/{}]",
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
	let output_file_id = output_file.get_id().unwrap();
	println!("{output_file_id:?}");
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
	Ok(())
}
