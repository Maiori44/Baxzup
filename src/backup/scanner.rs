use std::{path::{PathBuf, Path}, thread::{self, JoinHandle}, io};
use crate::config::config;
use flume::{Sender, SendError};

pub type Entry = (PathBuf, PathBuf);

fn scan_path(path: PathBuf, name: PathBuf, tx: &Sender<Entry>) -> Result<(), SendError<Entry>> {
	if path.is_file() {
		tx.send((path, name))?;
	}
	Ok(())
}

pub fn spawn_thread(tx: Sender<Entry>) -> JoinHandle<io::Result<()>> {
	thread::spawn(move || {
		let paths = config!(paths);
		for path_ref in paths {
			let path = path_ref.canonicalize()?;
			let name = Path::new(path.file_name().unwrap()).to_path_buf();
			if let Err(e) = scan_path(path, name, &tx) {
				return Err(io::Error::other(e.to_string()))
			}
		}
		Ok(())
	})
}