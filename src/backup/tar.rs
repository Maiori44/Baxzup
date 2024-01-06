use std::{thread::{JoinHandle, self}, io::{self, Write}, path::{PathBuf, Path}, fs::Metadata};
use crate::{config::{TagKeepMode, config}, error::ResultExt};
use super::bars::BarsHandler;
use colored::Colorize;
use tar::Builder;

trait GetSelf: Sized {
	fn get_self(self) -> Self {
		self
	}
}

impl GetSelf for Metadata {}

fn try_access<T: GetSelf>(path: &PathBuf, f: impl Fn(&Path) -> io::Result<T>) -> Option<T> {
	match f(path) {
		Ok(t) => Some(t),
		Err(e) => {
			let mut ignore = config!(ignore_unreadable_files).lock().unwrap();
			if *ignore {
				return None;
			}
			println!(
				"{} could not access '{}' ({e}). How to proceed? [{}etry/{}gnore/ignore {}ll]",
				"warning:".yellow().bold(),
				path.to_string_lossy().cyan().bold(),
				"R".cyan().bold(),
				"i".cyan().bold(),
				"a".cyan().bold(),
			);
			let mut choice = String::new();
			io::stdin().read_line(&mut choice).unwrap_or_exit();
			match choice.trim_start().as_bytes()[0].to_ascii_lowercase() {
				b'i' => None,
				b'a' => {
					*ignore = true;
					None
				}
				_ => {
					drop(ignore);
					try_access(path, f)
				}
			}
		}
	}
}

pub fn scan_path<T: GetSelf>(
	path: PathBuf,
	name: PathBuf,
	action: &mut impl FnMut(PathBuf, PathBuf) -> io::Result<()>,
	try_access: &impl Fn(&PathBuf, &dyn Fn(&Path) -> io::Result<dyn GetSelf>) -> Option<Box<dyn GetSelf>>,
) -> io::Result<()> {
	let config = config!();
	for pattern in &config.exclude {
		if pattern.is_match(path.as_os_str().as_encoded_bytes()) {
			return Ok(());
		}
	}

	macro_rules! try_access {
		(path.$f:ident()) => {
			match try_access(&path, &Path::$f) {
				Some(result) => result.get_self(),
				None => return Ok(()),
			}
		};
	}

	let meta = try_access!(path.metadata());
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
		action(path, name.clone())?;
		for entry in contents {
			let entry_path = entry.path().to_path_buf();
			scan_path(entry_path, name.join(entry.file_name()), action)?;
		}
	} else {
		action(path, name)?;
	}
	Ok(())
}

pub fn spawn_thread<W: Write + Send + 'static>(
	writer: W,
	bars_handler: &BarsHandler,
) -> JoinHandle<()> {
	let config = config!();
	let tar_bar = if config.progress_bars {
		Some(bars_handler.tar_bar.clone())
	} else {
		None
	};
	thread::spawn(move || {
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
			let name = Path::new(path.file_name().unwrap()).to_path_buf();
			if config.progress_bars {
				scan_path(path, name, &mut |path, name| {
					tar_bar.as_ref().unwrap().inc(1);
					builder.append_path_with_name(path, name)
				}, &try_access)
			} else {
				scan_path(path, name, &mut |path, name| {
					builder.append_path_with_name(path, name)
				}, &try_access)
			}.unwrap_or_exit();
		}
		if config.progress_bars {
			tar_bar.unwrap().finish_with_message("Archived ".green().bold().to_string());
		}
		builder.finish().unwrap_or_exit();
	})
}
