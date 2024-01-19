use crate::{backup::bars::{UNICODE_SPINNER, ASCII_SPINNER, PROGRESS_BAR}, input, error::ResultExt};
use sysinfo::{System, RefreshKind, CpuRefreshKind};
use toml::{toml, Value, Table};
use supports_unicode::Stream;
use colored::Colorize;
use std::io;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
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

pub fn get() -> Table {
	let system = System::new_with_specifics(
		RefreshKind::new()
			//.with_memory(MemoryRefreshKind::new().with_ram())
			.with_cpu(CpuRefreshKind::new())
	);
	let threads = system.cpus().len();
	let spinner_chars = if supports_unicode::on(Stream::Stderr) { UNICODE_SPINNER } else { ASCII_SPINNER };
	let mut config = toml! {
		[backup]
		paths = []
		exclude = ["?/cache/i"]
		exclude_tags = {"CACHEDIR.TAG" = "keep-tag"}
		follow_symlinks = false
		ignore_unreadable_files = false
		force_overwrite = false
		name = "%!hostname(%m-%y).tar.xz"

		[progress_bars]
		enable = true
		spinner_chars = spinner_chars
		progress_chars = PROGRESS_BAR

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

fn update_internal(config: &mut Table) {
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

pub fn update(
	msg: String,
	config: &mut Table,
	f: impl FnOnce(fn(&mut Table), &mut Table) -> io::Result<()>,
) -> io::Result<()> {
	if config.get("auto_update_config").is_some_and(|value| value.as_bool().unwrap_or_default()) {
		f(update_internal, config)
	} else {
		input!(format!(
			"{msg}\nUpdate configuration? [{}/{}]",
			"Y".cyan().bold(),
			"n".cyan().bold(),
		) => {
			b'n' => {},
			_ => f(update_internal, config)?,
		});
		Ok(())
	}
}
