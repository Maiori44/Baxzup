use std::{thread::{JoinHandle, self}, io::{self, Write}, path::{PathBuf, Path}, error::Error};
use crate::{config::{TagKeepMode, Config}, error::ResultExt};
use tar::Builder;

fn scan_path(
	path: PathBuf,
	name: PathBuf,
	builder: &mut Builder<impl Write>,
	config: &Config,
) -> Result<(), Box<dyn Error>> {
	for pattern in &config.exclude {
		if pattern.is_match(path.as_os_str().as_encoded_bytes()) {
			return Ok(());
		}
	}
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
		builder.append_path_with_name(path, name.clone())?;
		for entry in contents {
			let entry_path = entry.path().to_path_buf();
			scan_path(entry_path, name.join(entry.file_name()), builder, config)?;
		}
	}
	Ok(())
}

pub fn spawn_thread<W: Write + Send + 'static>(writer: W, config: Config) -> JoinHandle<io::Result<()>> {
	thread::spawn(move || {
		let mut builder = Builder::new(writer);
		for path_ref in &config.paths {
			let path = path_ref.canonicalize()?;
			let name = Path::new(path.file_name().unwrap()).to_path_buf();
			scan_path(path, name, &mut builder, &config).to_io_result()?;
		}
		Ok(())
	})
}
