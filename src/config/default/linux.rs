use crate::config::default::NAME;
use toml::{toml, Table};

pub fn get_config() -> Table {
	toml! {
		[backup]
		paths = ["/etc", "/home", "/root", "/usr", "/var"]
		exclude = ["cache"]
		exclude_tags = [["CACHEDIR.TAG", "keep-tag"]]
		progress_bars = true
		name = NAME
	}
}
