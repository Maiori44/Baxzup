use std::{thread::{JoinHandle, self}, hint, time::Duration, io::Read, ops::Deref, path::PathBuf};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use crate::config::config;
use super::tar::scan_path;
use xz2::read::XzEncoder;
use colored::Colorize;

pub struct InternalBarsHandler {
	pub xz_bar: ProgressBar,
	pub tar_bar: ProgressBar,
	_multi: MultiProgress,
	ticker: JoinHandle<()>,
	loader: JoinHandle<()>,
}

pub struct BarsHandler (Option<InternalBarsHandler>);

impl BarsHandler {
	pub fn new<R: Read>(compressor: *const XzEncoder<R>) -> Self {
		if !config!(progress_bars) {
			return Self(None);
		}
		let multi = MultiProgress::new();
		let tar_bar = ProgressBar::new(0).with_message("Archiving".cyan().bold().to_string()).with_style(
			ProgressStyle::with_template(
					"{msg}   {spinner} [{elapsed_precise}] {wide_bar:.yellow} {percent:>3}%"
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
		multi.set_move_cursor(true);
		let compressor_ptr = compressor as usize;
		Self(Some(InternalBarsHandler {
			xz_bar: xz_bar.clone(),
			tar_bar: tar_bar.clone(),
			_multi: multi,
			ticker: {
				let xz_bar = xz_bar.clone();
				thread::spawn(move || {
					let interval_duration = Duration::from_millis(166);
					// SAFETY: this awful hack is "fine" because the compressor is dropped after the thread.
					let compressor = unsafe { &*(compressor_ptr as *mut XzEncoder<R>) };
					loop {
						xz_bar.set_position(compressor.total_in());
						thread::sleep(interval_duration);
						if xz_bar.is_finished() {
							break;
						}
					}
				})
			},
			loader: thread::spawn(move || {
				let config = config!();
				for path_ref in &config.paths {
					if let Ok(path) = path_ref.canonicalize() {
						let _ = scan_path(path, PathBuf::new(), &mut |path, _| {
							if let Ok(meta) = if config.follow_symlinks {
								path.metadata()
							} else {
								path.symlink_metadata()
							} {
								xz_bar.inc_length(meta.len());
							}
							tar_bar.inc_length(1);
							Ok(())
						});
					}
				}
				xz_bar.inc_length(xz_bar.length().unwrap() / 30);
			}),
		}))
	}

	pub fn end(self) {
		// SAFETY: the program will always check Config.progress_bars before calling this.
		unsafe {
			let internal = self.0.unwrap_unchecked();
			internal.ticker.join().unwrap_unchecked();
			internal.loader.join().unwrap_unchecked();
		}
	}
}

impl Deref for BarsHandler {
	type Target = InternalBarsHandler;

	fn deref(&self) -> &InternalBarsHandler {
		debug_assert!(self.0.is_some());
		match self.0 {
			Some(ref bars_handler) => bars_handler,
			// SAFETY: the program will always check Config.progress_bars before calling this.
			None => unsafe { hint::unreachable_unchecked() },
		}
	}
}
