use std::{thread::{JoinHandle, self}, io::{self, Write}, path::{PathBuf, Path}, error::Error};
use crate::{config::{TagKeepMode, Config}, error::ResultExt};
use indicatif::ProgressBar;
use tar::Builder;

pub struct WriterObserver<W: Write + Send + 'static> {
	writer: W,
	bar: ProgressBar,
	total_wrote: f64,
	dirs_left: f64,
}

impl<W: Write + Send + 'static> Write for WriterObserver<W> {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		let wrote = self.writer.write(buf)?;
		self.total_wrote += buf.len() as f64;
		//self.bar.set_length((self.total_wrote * (self.dirs_left + 9.0).log10()) as u64);
		self.bar.set_length(self.total_wrote as u64);
		Ok(wrote)
	}

	fn flush(&mut self) -> io::Result<()> {
		self.writer.flush()
	}
}


fn scan_path(
	path: PathBuf,
	name: PathBuf,
	builder: &mut Builder<WriterObserver<impl Write + Send>>,
	bar: &ProgressBar,
	config: &Config,
) -> Result<(), Box<dyn Error>> {
	for pattern in &config.exclude {
		if pattern.is_match(path.as_os_str().as_encoded_bytes()) {
			return Ok(());
		}
	}
	//println!("{}", path.to_string_lossy());
	//bar.println(path.to_string_lossy());
	if path.is_file() {
		builder.append_path_with_name(path, name)?;
	} else if path.is_dir() {
		let mut contents = Vec::new();
		for entry in path.read_dir()? {
			let entry = entry?;
			if let Some(mode) = config.exclude_tags.get(&entry.file_name()).copied() {
				if mode == TagKeepMode::None {
					return Ok(())
				} else {
					contents.clear();
					if mode == TagKeepMode::Tag {
						contents.push(entry);
					}
				}
				break;
			}
			contents.push(entry);
		}
		builder.get_mut().dirs_left += 1.0;
		builder.append_path_with_name(path, name.clone())?; //TODO: refactor and do stuff manually
		for entry in contents {
			let entry_path = entry.path().to_path_buf();
			scan_path(entry_path, name.join(entry.file_name()), builder, bar, config)?;
		}
		builder.get_mut().dirs_left -= 1.0;
	}
	Ok(())
}

pub fn spawn_thread<W: Write + Send + 'static>(
	writer: W,
	config: Config,
	bar: ProgressBar,
) -> JoinHandle<io::Result<()>> {
	thread::spawn(move || {
		let mut builder = Builder::new(WriterObserver {
			writer,
			bar: bar.clone(),
			total_wrote: 0.0,
			dirs_left: 0.0,
		});
		for path_ref in &config.paths {
			let path = path_ref.canonicalize()?;
			let name = Path::new(path.file_name().unwrap()).to_path_buf();
			scan_path(path, name, &mut builder, &bar, &config).to_io_result()?;
		}
		builder.finish()?;
		Ok(())
	})
}
