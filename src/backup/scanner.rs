use std::{path::{PathBuf, Path}, thread::{self, JoinHandle}, io, error::Error};
use crate::{config::{config, TagKeepMode}, error::ResultExt};
use flume::Sender;
use regex::bytes::Regex;

pub type Entry = (PathBuf, PathBuf);

struct Scanner<'a> {
	tx: Sender<Entry>,
	exclude: &'a Vec<Regex>,
}

impl<'a> Scanner<'a> {
	fn scan_path(&mut self, path: PathBuf, name: PathBuf) -> Result<(), Box<dyn Error>> {
		//println!("scanner: {}", name.display().to_string());
		for pattern in self.exclude {
			if pattern.is_match(path.as_os_str().as_encoded_bytes()) {
				return Ok(());
			}
		}
		if path.is_file() {
			self.tx.send((path, name))?;
		} else if path.is_dir() {
			let tags = config!(exclude_tags);
			let mut contents = Vec::new();
			for entry in path.read_dir()? {
				let entry = entry?;
				if let Some(mode) = tags.get(&entry.file_name()).copied() {
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
			self.tx.send((path, name.clone()))?;
			for entry in contents {
				let entry_path = entry.path().to_path_buf();
				self.scan_path(entry_path, name.join(entry.file_name()))?;
			}
		}
		Ok(())
	}
}

pub fn spawn_thread(tx: Sender<Entry>) -> JoinHandle<io::Result<()>> {
	thread::spawn(move || {
		let (paths, exclude) = config!(paths, exclude);
		let mut scanner = Scanner { tx, exclude };
		for path_ref in paths {
			let path = path_ref.canonicalize()?;
			let name = Path::new(path.file_name().unwrap()).to_path_buf();
			scanner.scan_path(path, name).to_io_result()?;
			println!("finished {}", path_ref.display().to_string());
		}
		println!("scanner done!");
		Ok(())
	})
}
