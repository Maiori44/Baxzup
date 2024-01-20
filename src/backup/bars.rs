use std::{thread::{JoinHandle, self}, time::Duration, io::{Read, self, Write}, sync::{OnceLock, RwLock}, path::PathBuf};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use crate::config::config;
use super::{tar::scan_path, get_output_file_id};
use xz2::read::XzEncoder;
use colored::Colorize;

#[derive(Debug)]
pub struct BarsHandler {
	pub tar_bar: ProgressBar,
	pub xz_bar: ProgressBar,
	pub status_bar: ProgressBar,
	pub multi: MultiProgress,
	ticker: JoinHandle<()>,
	loader: JoinHandle<()>,
}

pub const UNICODE_SPINNER: &str = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ";
pub const ASCII_SPINNER: &str = "|/-\\ ";
pub const PROGRESS_BAR: &str = "█░";

static BARS_HANDLER: RwLock<OnceLock<BarsHandler>> = RwLock::new(OnceLock::new());

fn check_chars(name: &str, chars: &str) -> Result<(), String> {
	if chars.chars().count() < 2 {
		Err(format!(
			"`{}` must contain at least 2 characters",
			name.cyan().bold()
		))
	} else {
		Ok(())
	}
}

impl BarsHandler {
	pub fn init<R: Read>(compressor: *const XzEncoder<R>) -> Result<(), String> {
		if !*config!(progress_bars) {
			return Ok(());
		}
		let (spinner_chars, progress_chars) = config!(spinner_chars, progress_chars);
		check_chars("progress_bars.spinner_chars", spinner_chars)?;
		check_chars("progress_bars.progress_chars", progress_chars)?;
		let multi = MultiProgress::new();
		let tar_bar = ProgressBar::new(0).with_message("Archiving".cyan().bold().to_string()).with_style(
			ProgressStyle::with_template(
				"{msg}   {spinner} [{elapsed_precise}] {wide_bar:.yellow} {percent:>3}%"
			)
			.unwrap()
			.tick_chars(spinner_chars)
			.progress_chars(progress_chars)
		);
		let xz_bar = ProgressBar::new(0).with_message("Compressing".cyan().bold().to_string()).with_style(
			ProgressStyle::with_template(
				"{msg} {spinner} [{elapsed_precise}] {wide_bar:.magenta} {percent:>3}%"
			)
			.unwrap()
			.tick_chars(spinner_chars)
			.progress_chars(progress_chars)
		);
		let status_bar = ProgressBar::new(3).with_prefix("Last event".magenta().bold().to_string()).with_style(
			ProgressStyle::with_template(
				"{prefix}{bar:3.magenta.bold} [{elapsed_precise}] {wide_msg}"
			)
			.unwrap()
			.progress_chars(". ")
		);
		multi.add(tar_bar.clone());
		multi.add(xz_bar.clone());
		multi.add(status_bar.clone());
		status_bar.set_message("Starting...");
		multi.set_move_cursor(true);
		let compressor_ptr = compressor as usize;
		let bars_handler = Self {
			tar_bar: tar_bar.clone(),
			xz_bar: xz_bar.clone(),
			status_bar: status_bar.clone(),
			multi,
			ticker: {
				let xz_bar = xz_bar.clone();
				let status_bar = status_bar.clone();
				thread::spawn(move || {
					let interval_duration = Duration::from_millis(166);
					let mut counter = 0u8;
					let mut prev_out = 0;
					// SAFETY: this awful hack is "fine" because the compressor is dropped after the thread.
					let compressor = unsafe { &*(compressor_ptr as *mut XzEncoder<R>) };
					loop {
						xz_bar.set_position(compressor.total_in());
						if prev_out < compressor.total_out() {
							prev_out = compressor.total_out();
							status_bar.set_message(format!(
								"Writing {} compressed bytes",
								prev_out.to_string().cyan().bold()
							));
						}
						counter = counter.wrapping_add(1);
						if counter & 16 == 0 {
							xz_bar.suspend(|| {
								BarsHandler::redo_terminal();
							});
						}
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
						let _ = scan_path(
							get_output_file_id(config),
							path,
							PathBuf::new(),
							&|_, _| true,
							&mut |path, _| {
								let mut updated_bars = 0;
								if !xz_bar.is_finished() {
									if let Ok(meta) = if config.follow_symlinks {
										path.metadata()
									} else {
										path.symlink_metadata()
									} {
										xz_bar.inc_length(meta.len());
									}
									updated_bars += 1;
								}
								if !tar_bar.is_finished() {
									tar_bar.inc_length(1);
									updated_bars += 1;
								}
								if updated_bars == 0 {
									Err(io::Error::other(""))
								} else {
									Ok(())
								}
							}
						);
					}
				}
				xz_bar.inc_length(xz_bar.length().unwrap() / 30);
				status_bar.inc(1);
				status_bar.set_message("Finished scanning");
			}),
		};
		BARS_HANDLER.write().unwrap().set(bars_handler).unwrap();
		Ok(())
	}

	/// SAFETY: the caller must make sure `BARS_HANDLER` containts a value by checking `Config.progress_bars`
	pub unsafe fn exec<T>(f: impl FnOnce(&BarsHandler) -> T) -> T {
		let bars_handler = BARS_HANDLER.read().unwrap();
		debug_assert!(bars_handler.get().is_some());
		f(bars_handler.get().unwrap_unchecked())
	}

	pub fn end(f: impl FnOnce(&BarsHandler)) {
		if let Some(bars_handler) = BARS_HANDLER.write().unwrap().take() {
			f(&bars_handler);
			let thread_id = thread::current().id();
			if bars_handler.ticker.thread().id() != thread_id {
				bars_handler.ticker.join().unwrap();
			}
			if bars_handler.loader.thread().id() != thread_id {
				bars_handler.loader.join().unwrap();
			}
		}
	}

	pub fn redo_terminal() {
		print!("\x1b[2;1H\x1B[0J");
		let _ = io::stdout().flush();
	}
}
