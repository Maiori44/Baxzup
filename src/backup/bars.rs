use std::{thread::{JoinHandle, self}, time::Duration, io::Read, sync::OnceLock, path::PathBuf};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use crate::config::config;
use super::tar::scan_path;
use xz2::read::XzEncoder;
use colored::Colorize;

#[derive(Debug)]
pub struct BarsHandler {
	pub xz_bar: ProgressBar,
	pub tar_bar: ProgressBar,
	pub multi: MultiProgress,
	ticker: JoinHandle<()>,
	loader: JoinHandle<()>,
}

static mut BARS_HANDLER: OnceLock<BarsHandler> = OnceLock::new();

impl BarsHandler {
	pub fn init<R: Read>(compressor: *const XzEncoder<R>) {
		if !*config!(progress_bars) {
			return;
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
		let bars_handler = Self {
			xz_bar: xz_bar.clone(),
			tar_bar: tar_bar.clone(),
			multi: multi,
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
						let _ = scan_path(path, PathBuf::new(), &|_, _| true, &mut |path, _| {
							if !xz_bar.is_finished() {
								if let Ok(meta) = if config.follow_symlinks {
									path.metadata()
								} else {
									path.symlink_metadata()
								} {
									xz_bar.inc_length(meta.len());
								}
							}
							if !tar_bar.is_finished() {
								tar_bar.inc_length(1);
							}
							Ok(())
						});
					}
				}
				xz_bar.inc_length(xz_bar.length().unwrap() / 30);
			}),
		};
		// SAFETY: only the main thread calls this function
		unsafe {
			BARS_HANDLER.set(bars_handler).unwrap_unchecked()
		}
	}

	pub unsafe fn exec_unchecked<T>(f: impl FnOnce(&BarsHandler) -> T) -> T {
		let bars_handler = BARS_HANDLER.get();
		debug_assert!(bars_handler.is_some());
		f(bars_handler.unwrap_unchecked())
	}

	pub fn exec<T>(f: impl FnOnce(&BarsHandler) -> T) -> Option<T> {
		if *config!(progress_bars) {
			// SAFETY: BARS_HANDLER will always contain a value when Config.progress_bars is true
			unsafe {
				Some(BarsHandler::exec_unchecked(f))
			}
		} else {
			None
		}
	}

	pub fn end(f: impl FnOnce(&BarsHandler)) {
		if *config!(progress_bars) {
			// SAFETY: BARS_HANDLER will always contain a value when Config.progress_bars is true
			unsafe {
				let bars_handler = BARS_HANDLER.take();
				debug_assert!(bars_handler.is_some());
				let bars_handler = bars_handler.unwrap_unchecked();
				f(&bars_handler);
				bars_handler.ticker.join().unwrap_unchecked();
				bars_handler.loader.join().unwrap_unchecked();
			}
		}
	}
}
