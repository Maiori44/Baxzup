use std::{thread::{JoinHandle, self}, io::{self, Write}, path::{PathBuf, Path}};
use crate::{config::{TagKeepMode, config}, error::ResultExt, input};
use super::{bars::BarsHandler, metadata};
use colored::Colorize;
use fs_id::{FileID, GetID};
use tar::Builder;

fn failed_access(path: &Path, e: &io::Error) -> bool {
	let mut ignore = config!(ignore_unreadable_files).lock().unwrap();
	if *ignore {
		return true;
	}
	input!(format!(
		"{} could not access `{}` ({e})\nHow to proceed? [{}etry/{}gnore/ignore {}ll]",
		"warning:".yellow().bold(),
		path.to_string_lossy().cyan().bold(),
		"R".cyan().bold(),
		"i".cyan().bold(),
		"a".cyan().bold(),
	) => {
		b'a' => {
			*ignore = true;
			true
		},
		b'i' => true,
		_ => false,
	})
}

fn scan_path_internal(
	output_file_id: FileID,
	path: PathBuf,
	name: PathBuf,
	failed_access: fn(&Path, &io::Error) -> bool,
	action: &mut impl FnMut(&PathBuf, &PathBuf) -> io::Result<()>,
) {
	macro_rules! try_access {
		($f:expr) => {
			loop {
				match $f {
					Ok(result) => break result,
					Err(e) if failed_access(&path, &e) => return,
					_ => {}
				}
			}
		};
	}

	let config = config!();
	let meta = try_access!(metadata(&path));
	if meta.is_dir() && (config.follow_symlinks || !meta.is_symlink()) {
		let mut contents = Vec::new();
		let mut keep_tag = false;
		for entry in try_access!(path.read_dir()) {
			let entry = try_access!(entry);
			if let Some(mode) = config.exclude_tags.get(&entry.file_name()).copied() {
				if mode == TagKeepMode::None {
					return;
				} else {
					contents.clear();
					if mode == TagKeepMode::Tag {
						keep_tag = true;
						contents.push(entry);
					}
				}
				break;
			}
			contents.push(entry);
		}
		try_access!(action(&path, &name));
		let scan_func = if keep_tag { scan_path_internal } else { scan_path };
		for entry in contents {
			let entry_path = entry.path().to_path_buf();
			scan_func(output_file_id, entry_path, name.join(entry.file_name()), failed_access, action);
		}
	} else {
		if output_file_id == try_access!(path.get_id()) {
			return;
		}
		try_access!(action(&path, &name));
	}
}

pub fn scan_path(
	output_file_id: FileID,
	path: PathBuf,
	name: PathBuf,
	failed_access: fn(&Path, &io::Error) -> bool,
	action: &mut impl FnMut(&PathBuf, &PathBuf) -> io::Result<()>,
) {
	for pattern in config!(exclude) {
		if pattern.is_match(path.as_os_str().as_encoded_bytes()) {
			return;
		}
	}
	scan_path_internal(output_file_id, path, name, failed_access, action)
}

fn archive<'a, W: Write + Send + 'static>(
	writer: &mut W,
	output_file_id: FileID,
	paths: impl Iterator<Item = &'a PathBuf>,
) {
	let mut builder = Builder::new(writer);
	builder.follow_symlinks(*config!(follow_symlinks));
	for path_ref in paths {
		let path = path_ref.canonicalize().unwrap_or_exit();
		#[cfg(windows)]
		let name = match path.file_name() {
			Some(name) => Path::new(name).to_path_buf(),
			None => {
				use regex::Regex;
				let path_str = path.to_string_lossy();
				let drive = Regex::new(r"[A-Z]:")
					.unwrap()
					.find(&path_str)
					.unwrap()
					.as_str();
				let mut result = String::with_capacity(8);
				result.push_str("drive ");
				result.push_str(drive);
				Path::new(&result).to_path_buf()
			}
		};
		#[cfg(unix)]
		let name = Path::new(
			path.file_name().unwrap_or_else(|| std::ffi::OsStr::new("root"))
		).to_path_buf();
		if *config!(progress_bars) {
			scan_path(output_file_id, path, name, |path, e| {
				unsafe {
					BarsHandler::exec(|bars_handler| bars_handler.multi.suspend(|| {
						let ignore = failed_access(path, e);
						BarsHandler::redo_terminal();
						ignore
					}))
				}
			}, &mut |path, name| {
				unsafe {
					BarsHandler::exec(|bars_handler| {
						bars_handler.tar_bar.inc(1);
						bars_handler.status_bar.set_message(format!(
							"Archiving `{}`",
							path.display().to_string().cyan().bold()
						));
					});
				}
				builder.append_path_with_name(path, name)
			})
		} else {
			scan_path(output_file_id, path, name, failed_access, &mut |path, name| {
				println!(
					"Archiving `{}`",
					path.display().to_string().cyan().bold()
				);
				builder.append_path_with_name(path, name)
			})
		};
	}
	builder.finish().unwrap_or_exit();
}

pub fn spawn_thread<W: Write + Send + 'static>(
	mut writer: W,
	output_file_id: FileID
) -> JoinHandle<()> {
	thread::spawn(move || {
		let config = config!();
		if config.use_multiple_subarchives {
			todo!()
		} else {
			archive(&mut writer, output_file_id, config.paths.iter())
		}
		if config.progress_bars {
			unsafe {
				BarsHandler::exec(|bars_handler| {
					bars_handler.status_bar.inc(1);
					bars_handler.status_bar.set_message("Finished archiving");
					bars_handler.tar_bar.finish_with_message("Archived ".green().bold().to_string());
				})
			}
		} else {
			println!("Finished archiving...");
		}
		/*let config = config!();
		let mut builder = Builder::new(writer);
		builder.follow_symlinks(config.follow_symlinks);
		for path_ref in &config.paths {
			let path = path_ref.canonicalize().unwrap_or_exit();
			#[cfg(windows)]
			let name = match path.file_name() {
				Some(name) => Path::new(name).to_path_buf(),
				None => {
					use regex::Regex;
					let path_str = path.to_string_lossy();
					let drive = Regex::new(r"[A-Z]:")
						.unwrap()
						.find(&path_str)
						.unwrap()
						.as_str();
					let mut result = String::with_capacity(8);
					result.push_str("drive ");
					result.push_str(drive);
					Path::new(&result).to_path_buf()
				}
			};
			#[cfg(unix)]
			let name = Path::new(
				path.file_name().unwrap_or_else(|| std::ffi::OsStr::new("root"))
			).to_path_buf();
			if config.progress_bars {
				scan_path(output_file_id, path, name, &|path, e| {
					unsafe {
						BarsHandler::exec(|bars_handler| bars_handler.multi.suspend(|| {
							let ignore = failed_access(path, e);
							BarsHandler::redo_terminal();
							ignore
						}))
					}
				}, &mut |path, name| {
					unsafe {
						BarsHandler::exec(|bars_handler| {
							bars_handler.tar_bar.inc(1);
							bars_handler.status_bar.set_message(format!(
								"Archiving `{}`",
								path.display().to_string().cyan().bold()
							));
						});
					}
					builder.append_path_with_name(path, name)
				})
			} else {
				scan_path(output_file_id, path, name, &failed_access, &mut |path, name| {
					println!(
						"Archiving `{}`",
						path.display().to_string().cyan().bold()
					);
					builder.append_path_with_name(path, name)
				})
			};
		}
		if config.progress_bars {
			unsafe {
				BarsHandler::exec(|bars_handler| {
					bars_handler.status_bar.inc(1);
					bars_handler.status_bar.set_message("Finished archiving");
					bars_handler.tar_bar.finish_with_message("Archived ".green().bold().to_string());
				})
			}
		} else {
			println!("Finished archiving...");
		}
		builder.finish().unwrap_or_exit();*/
	})
}
