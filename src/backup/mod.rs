use indicatif::{ProgressBar, ProgressStyle};
use xz2::{read::XzEncoder, stream::MtStreamBuilder};
use std::{io::{self, Read}, fs::File, thread::{self, JoinHandle}, time::Duration};
use crate::{config::Config, error::ResultExt};

mod tar;

fn spawn_bar_thread<R: Read>(bar: ProgressBar, compressor: *const XzEncoder<R>) -> JoinHandle<()> {
	//SAFETY: This awful hack is "fine" because the compressor will only go out of scope after the thread ended.
	unsafe { //TODO: I should find a better way to do this...
		let compressor_ptr = compressor as usize;
		thread::spawn(move || {
			let compressor = &*(compressor_ptr as *const XzEncoder<R>);
			loop {
				//bar.println(format!("TOTAL OUT {}/{}", (*compressor).total_out(), bar.length().unwrap()));
				bar.set_position(compressor.total_out());
				thread::sleep(Duration::from_millis(166));
				if bar.is_finished() {
					break;
				}
			}
		})
	}
}

pub fn init(config: Config) -> io::Result<()> {
	let bar = ProgressBar::new(0).with_style(
		ProgressStyle::with_template("{spinner} [{elapsed_precise}] {wide_bar} {percent}%")
			.unwrap()
	);
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
	let mut output_file = File::options().read(true).write(true).create_new(true).open(&config.name)?;
	let tar_thread = tar::spawn_thread(writer, config, bar.clone());
	let bar_thread = spawn_bar_thread(bar.clone(), &compressor);
	io::copy(&mut compressor, &mut output_file)?;
	bar.finish();
	bar_thread.join().unwrap();
	tar_thread.join().unwrap()
}
