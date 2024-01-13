use std::{fs::File, os::fd::{FromRawFd, AsRawFd}, io::{self, Read, Write}, thread};
use sysinfo::{System, RefreshKind, CpuRefreshKind};
use toml::{toml, Value, Table};

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as specifics;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as specifics;

#[cfg(not(any(
	target_os = "windows",
	target_os = "macos",
)))]
mod linux;
#[cfg(not(any(
	target_os = "windows",
	target_os = "macos",
)))]
use linux as specifics;

fn test_utf8_chars(test: &str) -> io::Result<bool> {
	println!("\x1b[6n");
	let mut prefix = [0; 2];
	let mut stdin = unsafe{File::from_raw_fd(io::stdin().as_raw_fd())};
	thread::spawn(|| {
		let mut stdin = unsafe{File::from_raw_fd(io::stdin().as_raw_fd())};
		loop {
			println!("{:?}", stdin.flush());
			thread::sleep(std::time::Duration::from_millis(1000));
		}
	});
	stdin.read(&mut prefix)?;
	println!("'{:?}'", prefix);
	for ch in test.chars() {
		debug_assert!(ch.len_utf8() > 1);
		
	}
	Ok(true)
}

pub fn get() -> Table {
	let system = System::new_with_specifics(
		RefreshKind::new()
			//.with_memory(MemoryRefreshKind::new().with_ram())
			.with_cpu(CpuRefreshKind::new())
	);
	let threads = system.cpus().len();
	let ascii_spinner = test_utf8_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏").unwrap_or(true);
	let ascii_bar = test_utf8_chars("█░").unwrap_or(true);
	let mut config = toml! {
		[backup]
		paths = []
		exclude = ["?/cache/i"]
		exclude_tags = [["CACHEDIR.TAG", "keep-tag"]]
		follow_symlinks = false
		ignore_unreadable_files = false
		force_overwrite = false
		name = "%!hostname(%m-%y).tar.xz"

		[progress_bars]
		enable = true
		ascii_spinner = false
		ascii_bar = false

		[xz]
		level = 8
		threads = threads
		block_size = 0
	};
	let specifics = specifics::get();
	for table_key in specifics.keys() {
		let table = config[table_key].as_table_mut().unwrap();
		for (key, value) in specifics[table_key].as_table().unwrap() {
			match table.get_mut(key) {
				Some(Value::Array(array)) if value.is_array() => {
					array.extend_from_slice(value.as_array().unwrap());
				}
				Some(Value::Table(table)) if value.is_table() => {
					table.extend(value.as_table().unwrap().clone());
				}
				_ => {
					table.insert(key.to_string(), value.clone());
				}
			}
		}
	}
	config
}

pub fn update(config: &mut Table) {
	let mut default = get();
	for table_key in config.keys() {
		let (Some(default_table), Some(table)) = (
			default.get_mut(table_key).map(|v| v.as_table_mut().unwrap()),
			config[table_key].as_table()
		) else {
			default.insert(table_key.into(), config[table_key].clone());
			continue;
		};
		for (key, value) in table {
			default_table.insert(key.into(), value.clone());
		}
	}
	*config = default;
}
