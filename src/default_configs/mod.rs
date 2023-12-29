use sysinfo::{System, RefreshKind, MemoryRefreshKind, CpuRefreshKind};
use toml::toml;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
use windows::get_config;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos::get_config;

#[cfg(not(any(
	target_os = "windows",
	target_os = "macos",
)))]
mod linux;
#[cfg(not(any(
	target_os = "windows",
	target_os = "macos",
)))]
use linux::get_config;

pub fn get() -> String {
	let system = System::new_with_specifics(
		RefreshKind::new()
			.with_memory(MemoryRefreshKind::new().with_ram())
			.with_cpu(CpuRefreshKind::new())
	);
	let memlimit = system.total_memory() / 2;
	let threads = system.cpus().len();
	let mut config = get_config();
	config.extend(toml! {
		[xz]
		level = 9
		memlimit = memlimit
		threads = threads
		block-size = 0
	});
	config.to_string()
}
