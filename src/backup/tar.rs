use std::{
	fs::{DirEntry, File},
	io::{self, Seek, SeekFrom, Write},
	path::{Path, PathBuf},
	ptr, thread::{self, JoinHandle}
};
use crate::{config::{TagKeepMode, config}, error::ResultExt, input, static_ptr::StaticPointer};
use super::{bars::BarsHandler, metadata, BorrowCompressor};
use colored::Colorize;
use fs_id::{FileID, GetID};
use os_pipe::PipeReader;
use tar::{Builder, EntryType, Header};

macro_rules! try_access {
	($path:expr, $f:expr, $else:expr, $failed:expr) => {
		loop {
			match $f {
				Ok(result) => break result,
				Err(e) if $failed(&$path, &e) => $else,
				_ => {}
			}
		}
	};
	($path:expr, $f:expr, $failed:expr) => {
		self::try_access!($path, $f, return, $failed)
	};
}

use try_access;

pub struct SubarchiveValues {
	pub reader: PipeReader,
	pub dir_path: *const PathBuf,
	pub name_start: *const Option<PathBuf>,
	pub builder: *mut Builder<File>,
	pub f: fn(&mut dyn BorrowCompressor, &PathBuf, &Option<PathBuf>, &mut Builder<File>) -> io::Result<()>
}

pub static mut SUBARCHIVE_VALUES: StaticPointer<SubarchiveValues> = StaticPointer::null();

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

fn get_dir_contents(
	path: &PathBuf,
	failed_access: &fn(&Path, &io::Error) -> bool,
) -> Option<(Vec<DirEntry>, bool)> {
	macro_rules! try_access {
		($f:expr) => {
			self::try_access!(path, $f, return None, &failed_access)
		};
	}

	let mut contents = Vec::new();
	let mut keep_tag = false;
	for entry in try_access!(path.read_dir()) {
		let entry = try_access!(entry);
		if let Some(mode) = config!(exclude_tags).get(&entry.file_name()).copied() {
			if mode == TagKeepMode::None {
				return None;
			}
			contents.clear();
			if mode == TagKeepMode::Tag {
				keep_tag = true;
				contents.push(entry);
			}
			break;
		}
		contents.push(entry);
	}
	Some((contents, keep_tag))
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
			self::try_access!(path, $f, failed_access)
		};
	}

	let config = config!();
	let meta = try_access!(metadata(&path));
	if meta.is_dir() && (config.follow_symlinks || !meta.is_symlink()) {
		let Some((contents, keep_tag)) = get_dir_contents(&path, &failed_access) else {
			return;
		};
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

fn is_excluded(path: &[u8]) -> bool {
	for pattern in config!(exclude) {
		if pattern.is_match(path) {
			return true;
		}
	}
	false
}

pub fn scan_path(
	output_file_id: FileID,
	path: PathBuf,
	name: PathBuf,
	failed_access: fn(&Path, &io::Error) -> bool,
	action: &mut impl FnMut(&PathBuf, &PathBuf) -> io::Result<()>,
) {
	if is_excluded(path.as_os_str().as_encoded_bytes()) {
		return
	}
	scan_path_internal(output_file_id, path, name, failed_access, action)
}

#[cfg(windows)]
fn get_name(path: &Path, name_start: &Option<PathBuf>) -> PathBuf {
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
	match name_start {
		Some(name_start) => name_start.join(name),
		None => name,
	}
}

#[cfg(unix)]
fn get_name(path: &Path, name_start: &Option<PathBuf>) -> PathBuf {
	let name = Path::new(
		path.file_name().unwrap_or_else(|| std::ffi::OsStr::new("root"))
	).to_path_buf();
	match name_start {
		Some(name_start) => name_start.join(name),
		None => name,
	}
}

fn archive_internal<'a, W: Write + Send + 'static>(
	builder: &mut Builder<W>,
	output_file_id: FileID,
	paths: impl Iterator<Item = impl AsRef<Path>>,
	name_start: &Option<PathBuf>,
	failed_access: fn(&Path, &io::Error) -> bool,
) {
	'main: for path_ref in paths {
		let path_ref = path_ref.as_ref();
		let path = try_access!(path_ref, path_ref.canonicalize(), continue 'main, failed_access);
		let name = get_name(&path, name_start);
		if *config!(progress_bars) {
			scan_path(output_file_id, path, name, failed_access, &mut |path, name| {
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
}

fn archive<'a, W: Write + Send + 'static>(
	writer: W,
	output_file_id: FileID,
	paths: impl Iterator<Item = impl AsRef<Path>>,
	failed_access: fn(&Path, &io::Error) -> bool,
) -> Builder<W> {
	let mut builder = Builder::new(writer);
	builder.follow_symlinks(*config!(follow_symlinks));
	archive_internal(&mut builder, output_file_id, paths, &None, failed_access);
	builder.finish().unwrap_or_exit();
	builder
}

fn make_subarchives<W: Write + Send + 'static>(
	mut builder: Builder<W>,
	output_file_id: FileID,
	main_thread: &thread::Thread,
	paths: &Vec<PathBuf>,
	name_start: Option<PathBuf>,
	failed_access: fn(&Path, &io::Error) -> bool,
) {
	macro_rules! try_access {
		($path:expr, $f:expr) => {
			self::try_access!($path, $f, return, failed_access)
		};
	}

	let mut root_files = Vec::new();
	let mut root_dirs = Vec::with_capacity(paths.len());
	for path_ref in paths {
		if is_excluded(path_ref.as_os_str().as_encoded_bytes()) {
			continue
		}
		if path_ref.is_dir() {
			root_dirs.push(path_ref);
		} else {
			root_files.push(path_ref);
		}
	}
	println!("{root_files:?}\n{root_dirs:?}");
	archive_internal(&mut builder, output_file_id, root_files.into_iter(), &name_start, failed_access);
	if root_dirs.len() == 1 {
		let path = try_access!(&paths[0], paths[0].canonicalize());
		let mut inner_paths = Vec::new();
		let name_start = Some(get_name(path.as_path(), &name_start));
		for entry in try_access!(path, path.read_dir()) {
			inner_paths.push(try_access!(path, entry).path());
		}
		make_subarchives(
			builder,
			output_file_id,
			main_thread,
			&inner_paths,
			name_start,
			failed_access
		);
	} else {
		for dir_path in root_dirs {
			let Some((contents, ..)) = get_dir_contents(dir_path, &failed_access) else {
				return;
			};
			let (reader, writer) = os_pipe::pipe().unwrap_or_exit();
			let subarchive_values = SubarchiveValues {
				reader,
				dir_path,
				name_start: &name_start,
				builder: &mut builder as *mut _ as usize as *mut Builder<File>,
				f: |mut compressor, dir_path, name_start, builder| {
					let mut header = Header::new_gnu();
					header.set_metadata(&dir_path.metadata()?);
					header.set_mode(header.mode().unwrap() ^ 0o140000);
					header.set_entry_type(EntryType::Regular);
					let path_name = get_name(&dir_path, &name_start);
					let header_pos = builder.get_mut().stream_position()?;
					builder.append_data(
						&mut header,
						path_name.with_extension("tar.xz"),
						&mut compressor,
					)?;
					header.set_size(compressor.borrow_compressor().total_out());
					header.set_cksum();
					let output_file = builder.get_mut();
					output_file.seek(SeekFrom::Start(header_pos))?;
					output_file.write(header.as_bytes())?;
					output_file.seek(SeekFrom::End(0))?;
					Ok(())
				}
			};
			// SAFETY: Recieving thread is parked.
			unsafe { SUBARCHIVE_VALUES.set(&subarchive_values) }
			main_thread.unpark();
			archive(writer, output_file_id, contents.into_iter().map(|entry| entry.path()), failed_access);
			thread::park();
		}
		builder.finish().unwrap_or_exit();
	}
}

pub fn spawn_thread<W: Write + Send + 'static>(
	writer: W,
	output_file_id: FileID
) -> JoinHandle<()> {
	let config = config!();
	let main_thread = config.use_multiple_subarchives.then(thread::current);
	thread::spawn(move || {
		let failed_access: Box<fn(&Path, &io::Error) -> bool> = if config.progress_bars {
			Box::new(|path, e| {
				unsafe {
					BarsHandler::exec(|bars_handler| bars_handler.multi.suspend(|| {
						let ignore = failed_access(path, e);
						BarsHandler::redo_terminal();
						ignore
					}))
				}
			})
		} else { Box::new(failed_access) };
		if let Some(main_thread) = main_thread {
			let mut builder = Builder::new(writer);
			builder.follow_symlinks(*config!(follow_symlinks));
			make_subarchives(builder, output_file_id, &main_thread, &config.paths, None, *failed_access);
			// SAFETY: Recieving thread is parked.
			unsafe { SUBARCHIVE_VALUES.set(ptr::null()) }
			main_thread.unpark();
		} else {
			archive(writer, output_file_id, config.paths.iter(), *failed_access);
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
