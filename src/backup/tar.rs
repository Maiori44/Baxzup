use std::{thread::{JoinHandle, self}, io::{self, Write}, path::{PathBuf, Path}};
use crate::{config::{TagKeepMode, config}, error::ResultExt, input};
use super::{bars::BarsHandler, OutputFileID, get_output_file_id};
use colored::Colorize;
use tar::Builder;

fn failed_access(path: &Path, e: &io::Error) -> bool {
	let mut ignore = config!(ignore_unreadable_files).lock().unwrap();
	if *ignore {
		return true;
	}
	input!(format!(
		"{} could not access '{}' ({e})\nHow to proceed? [{}etry/{}gnore/ignore {}ll]",
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

pub fn scan_path(
	output_file_id: OutputFileID,
	path: PathBuf,
	name: PathBuf,
	failed_access: &impl Fn(&Path, &io::Error) -> bool,
	action: &mut impl FnMut(&PathBuf, &PathBuf) -> io::Result<()>,
) -> io::Result<()> {
	let config = config!();
	for pattern in &config.exclude {
		if pattern.is_match(path.as_os_str().as_encoded_bytes()) {
			return Ok(());
		}
	}

	macro_rules! try_access {
		($f:expr) => {
			loop {
				match $f {
					Ok(result) => break result,
					Err(e) if failed_access(&path, &e) => return Ok(()),
					_ => {}
				}
			}
		};
	}

	let meta = try_access!(if config.follow_symlinks {
		path.metadata()
	} else {
		path.symlink_metadata()
	});
	#[cfg(not(target_os = "windows"))]
	{
		use std::os::unix::fs::MetadataExt;

		if output_file_id == (meta.dev(), meta.ino()) {
			return Ok(());
		}
	}
	if meta.is_dir() && (config.follow_symlinks || !meta.is_symlink()) {
		let mut contents = Vec::new();
		for entry in try_access!(path.read_dir()) {
			let entry = entry?;
			if let Some(mode) = config.exclude_tags.get(&entry.file_name()).copied() {
				if mode == TagKeepMode::None {
					return Ok(())
				} else {
					contents.clear();
					if mode == TagKeepMode::Tag {
						contents.push(entry);
					}
				}
				break;
			}
			contents.push(entry);
		}
		try_access!(action(&path, &name));
		for entry in contents {
			let entry_path = entry.path().to_path_buf();
			scan_path(output_file_id, entry_path, name.join(entry.file_name()), failed_access, action)?;
		}
	} else {
		try_access!(action(&path, &name));
	}
	Ok(())
}

pub fn spawn_thread<W: Write + Send + 'static>(writer: W) -> JoinHandle<()> {
	thread::spawn(move || {
		let config = config!();
		let mut builder = Builder::new(writer);
		builder.follow_symlinks(config.follow_symlinks);
		for path_ref in &config.paths {
			let path = path_ref.canonicalize().unwrap_or_exit();
			#[cfg(target_os = "windows")]
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
			#[cfg(not(target_os = "windows"))]
			let name = Path::new(path.file_name().unwrap_or_else(|| std::ffi::OsStr::new("root"))).to_path_buf();
			let output_file_id = get_output_file_id(config);
			if config.progress_bars {
				scan_path(output_file_id, path, name, &|path, e| {
					// SAFETY: BARS_HANDLER will always contain a value when Config.progress_bars is true
					unsafe {
						BarsHandler::exec_unchecked(|bars_handler| bars_handler.multi.suspend(|| {
							let ignore = failed_access(path, e);
							BarsHandler::redo_terminal();
							ignore
						}))
					}
				}, &mut |path, name| {
					BarsHandler::exec(|bars_handler| {
						bars_handler.tar_bar.inc(1);
					});
					builder.append_path_with_name(path, name)
				})
			} else {
				scan_path(output_file_id, path, name, &failed_access, &mut |path, name| {
					builder.append_path_with_name(path, name)
				})
			}.unwrap_or_exit();
		}
		BarsHandler::exec(|bars_handler| {
			bars_handler.tar_bar.finish_with_message("Archived ".green().bold().to_string());
		});
		builder.finish().unwrap_or_exit();
	})
}
