use std::{path::{PathBuf, Path}, thread::{self, JoinHandle}, io, error::Error};
use crate::{config::{config, TagKeepMode}, error::ResultExt};
use flume::Sender;
use regex::bytes::Regex;

pub type Entry = (PathBuf, PathBuf);

fn scan_path(
	path: PathBuf,
	name: PathBuf,
	tx: &Sender<Entry>,
	exclude: &Vec<Regex>
) -> Result<(), Box<dyn Error>> {
	for pattern in exclude {
		if pattern.is_match(path.as_os_str().as_encoded_bytes()) {
			return Ok(());
		}
	}
	if path.is_file() {
		tx.send((path, name))?;
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
		tx.send((path, name.clone()))?;
		for entry in contents {
			let entry_path = entry.path().to_path_buf();
			scan_path(entry_path, name.join(entry.file_name()), tx, exclude)?;
		}
	}
	Ok(())
}

pub fn spawn_thread(tx: Sender<Entry>) -> JoinHandle<io::Result<()>> {
	thread::spawn(move || {
		let (paths, exclude) = config!(paths, exclude);
		for path_ref in paths {
			let path = path_ref.canonicalize()?;
			let name = Path::new(path.file_name().unwrap()).to_path_buf();
			scan_path(path, name, &tx, exclude).to_io_result()?;
		}
		Ok(())
	})
}