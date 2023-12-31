use std::{thread::{JoinHandle, self}, io::{self, Write}};
use super::scanner;
use flume::Sender;
use os_pipe::PipeWriter;
use tar::Builder;

pub fn spawn_thread<'a>(writer: PipeWriter) -> JoinHandle<io::Result<()>> {
	thread::spawn(move || {
		let (tx, rx) = flume::unbounded();
		let scanner_thread = scanner::spawn_thread(tx);
		let mut builder = Builder::new(writer);
		loop {
			if let Ok((path, name)) = rx.try_recv() {
				//println!("{}", path.to_string_lossy().to_string());
				builder.append_path_with_name(path, name)?;
			} else if scanner_thread.is_finished() {
				builder.finish()?;
				break scanner_thread.join().unwrap();
			}
		}
	})
}

/*pub struct ChannelWriter {
	tx: Sender<Vec<u8>>
}

impl ChannelWriter {
	fn wait(&self) {
		loop {
			if self.tx.is_empty() {
				break;
			}
		}
	}
}

impl From<Sender<Vec<u8>>> for ChannelWriter {
	fn from(value: Sender<Vec<u8>>) -> Self {
		Self { tx: value }
	}
}

impl Write for ChannelWriter {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		//println!("write {} bytes", buf.len());
		if self.tx.is_disconnected() {
			return Ok(0);
		}
		self.tx.send(buf.to_vec()).to_io_result()?;
		Ok(buf.len())
	}

	fn flush(&mut self) -> io::Result<()> {
		Ok(())
	}
}*/
