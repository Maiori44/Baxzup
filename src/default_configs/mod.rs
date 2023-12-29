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
    unimplemented!()
}
