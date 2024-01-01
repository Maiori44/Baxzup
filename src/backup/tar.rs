use std::{thread::{JoinHandle, self}, io::{self, Write}};
use super::scanner;
use flume::Sender;
use tar::Builder;

pub fn spawn_thread<W: Write + Send + 'static>(writer: W) -> JoinHandle<io::Result<()>> {
	thread::spawn(move || {
		let (tx, rx) = flume::bounded(1);
		let scanner_thread = scanner::spawn_thread(tx);
		let mut builder = Builder::new(writer);
		loop {
			if let Ok((path, name)) = rx.try_recv() {
				println!("tar: {}", path.to_string_lossy().to_string());
				builder.append_path_with_name(path, name)?;
			} else if scanner_thread.is_finished() {
				builder.finish()?;
				break scanner_thread.join().unwrap();
			}
		}
	})
}

pub struct ChannelWriter {
	tx: Sender<Vec<u8>>
}

impl From<Sender<Vec<u8>>> for ChannelWriter {
	fn from(value: Sender<Vec<u8>>) -> Self {
		Self { tx: value }
	}
}

impl Write for ChannelWriter {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		match self.tx.send(buf.iter().rev().copied().collect()) {
			Ok(()) => Ok(buf.len()),
			Err(_) => Ok(0),
		}
	}

	fn flush(&mut self) -> io::Result<()> {
		Ok(())
	}
}
