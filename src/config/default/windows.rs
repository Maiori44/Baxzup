use crate::config::default::NAME;
use toml::{toml, Table};

pub fn get_config() -> Table {
	toml! {
		[backup]
		paths = ["C:\\"]
		exclude = ["C:\\\\Program Files", "C:\\\\Program Files \\(x86\\)", "C:\\\\Windows"]
		exclude_tags = [["CACHEDIR.TAG", "keep-tag"]]
		progress_bars = true
		name = NAME
	}
}
