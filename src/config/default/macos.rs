use crate::config::default::NAME;
use toml::{toml, Table};

pub fn get_config() -> Table {
	toml! {
		[backup]
		paths = ["/Users", "/Applications", "/Library"]
		exclude = ["cache"]
		exclude_tags = [["CACHEDIR.TAG", "keep-tag"]]
		progress_bars = true
		name = NAME
	}
}
