use sysinfo::{System, RefreshKind, CpuRefreshKind};
use toml::{toml, Value};

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

pub fn get() -> String {
	let system = System::new_with_specifics(
		RefreshKind::new()
			//.with_memory(MemoryRefreshKind::new().with_ram())
			.with_cpu(CpuRefreshKind::new())
	);
	let threads = system.cpus().len();
	let mut config = toml! {
		[backup]
		paths = []
		exclude = ["?/cache/i"]
		exclude_tags = [["CACHEDIR.TAG", "keep-tag"]]
		progress_bars = true
		follow_symlinks = false
		ignore_unreadable_files = false
		force_overwrite = false
		name = "%!hostname(%m-%y).tar.xz"

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
	config.to_string()
}
