use std::{io::{self, Read, Write, BufReader, BufRead}, fs::File};
use flume::Receiver;
use xz2::{read::XzEncoder, stream::MtStreamBuilder};
use crate::{config::config, error::ResultExt};

mod scanner;
mod tar;

pub fn init() -> io::Result<()> {
	let config = config!();
	let (reader, writer) = os_pipe::pipe()?;
	let tar_thread = tar::spawn_thread(writer);
	let mut compressor = XzEncoder::new_stream(
		reader,
		MtStreamBuilder::new()
			.preset(config.level)
			.threads(config.threads)
			.block_size(10)
			.encoder()
			.to_io_result()?
	);
	let mut output = File::options().read(true).write(true).create_new(true).open(config!(name))?;
	let mut buf = [0; 10240];
	loop {
		let read = compressor.read(&mut buf)?;
		println!("READ:            {read}");
		if read == 0 {
			break tar_thread.join().unwrap();
		}
		output.write(&mut buf[..read])?;
	}
}

/*pub struct ChannelBufReader {
	rx: Receiver<Vec<u8>>,
	current: Vec<u8>,
	pos: usize,
}

impl ChannelBufReader {
	fn check_current(&mut self) {
		if self.pos >= self.current.len() {
			if let Ok(new) = self.rx.recv() {
				self.current = new;
				self.pos = 0;
			}
		}
	}

	fn wait(&self) {
		loop {
			if self.rx.is_empty() {
				break;
			}
		}
	}
}

impl From<Receiver<Vec<u8>>> for ChannelBufReader {
	fn from(value: Receiver<Vec<u8>>) -> Self {
		Self { rx: value, current: Vec::new(), pos: 0 }
	}
}

impl Read for ChannelBufReader {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		unreachable!("XzEncoder never calls read...");
		/*if self.rx.is_disconnected() {
			return Ok(0)
		}
		for i in 0..buf.len() {
			self.check_current()?;
			buf[i] = self.current[self.pos];
			self.pos += 1;
		}
		println!("{buf:?}");
		Ok(buf.len())*/
	}
}

impl BufRead for ChannelBufReader {
	fn fill_buf(&mut self) -> io::Result<&[u8]> {
		if !self.rx.is_disconnected() {
			self.check_current();
		}
		Ok(&self.current[self.pos..])
	}

	fn consume(&mut self, amt: usize) {
		//println!("{amt} {}", self.current.len());
		self.pos += amt;
	}
}*/
