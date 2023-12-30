use std::{thread::{JoinHandle, self}, io};
use os_pipe::PipeWriter;
use tar::Builder;

use super::scanner;

pub fn spawn_thread(writer: PipeWriter) -> JoinHandle<io::Result<()>> {
	thread::spawn(move || {
		let (tx, rx) = flume::unbounded();
		let scanner_thread = scanner::spawn_thread(tx);
		let mut builder = Builder::new(writer);
		loop {
			if let Ok((path, name)) = rx.try_recv() {
				builder.append_path_with_name(path, name)?;
			} else if scanner_thread.is_finished() {
				break scanner_thread.join().unwrap();
			}
		}
	})
}