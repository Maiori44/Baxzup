use std::{thread::{JoinHandle, self}, hint, time::Duration, io::Read, ops::Deref};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use xz2::read::XzEncoder;
use colored::Colorize;

pub struct InternalBarsHandler {
	_multi: MultiProgress,
	pub xz_bar: ProgressBar,
	pub tar_bar: ProgressBar,
	pub thread: JoinHandle<()>
}

pub struct BarsHandler {
	pub enabled: bool,
	internal: Option<InternalBarsHandler>,
}

impl BarsHandler {
	pub fn new<R: Read>(enabled: bool, compressor: *const XzEncoder<R>) -> Self {
		if !enabled {
			return Self {
				enabled,
				internal: None
			};
		}
		let multi = MultiProgress::new();
		let tar_bar = ProgressBar::new(0).with_message("Archiving".cyan().bold().to_string()).with_style(
			ProgressStyle::with_template(
					"{msg}   {spinner} [{elapsed_precise}] {wide_bar:.yellow} {percent:>2}%"
				)
				.unwrap()
		);
		let xz_bar = ProgressBar::new(0).with_message("Compressing".cyan().bold().to_string()).with_style(
			ProgressStyle::with_template(
					"{msg} {spinner} [{elapsed_precise}] {wide_bar:.magenta} {percent:>3}%"
				)
				.unwrap()
		);
		multi.add(tar_bar.clone());
		multi.add(xz_bar.clone());
		let compressor_ptr = compressor as usize;
		Self {
			enabled,
			internal: Some(InternalBarsHandler {
				_multi: multi,
				xz_bar: xz_bar.clone(),
				tar_bar: tar_bar.clone(),
				thread: thread::spawn(move || {
					let interval_duration = Duration::from_millis(166);
					// SAFETY: this awful hack is "fine" because the compressor is dropped after the thread.
					let compressor = unsafe { &*(compressor_ptr as *mut XzEncoder<R>) };
					loop {
						xz_bar.set_position(compressor.total_out());
						tar_bar.tick();
						thread::sleep(interval_duration);
						if xz_bar.is_finished() && tar_bar.is_finished() {
							break;
						}
					}
				})
			})
		}
	}

	pub fn end(self) {
		// SAFETY: the program will always check self.enabled before calling this.
		unsafe { self.internal.unwrap_unchecked().thread.join().unwrap_unchecked() }
	}
}

impl Deref for BarsHandler {
	type Target = InternalBarsHandler;

	fn deref(&self) -> &InternalBarsHandler {
		debug_assert!(self.enabled);
		match self.internal {
			Some(ref bars_handler) => bars_handler,
			// SAFETY: the program will always check self.enabled before calling this.
			None => unsafe { hint::unreachable_unchecked() },
		}
	}
}
