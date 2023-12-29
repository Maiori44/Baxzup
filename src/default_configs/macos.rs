use crate::default_configs::NAME;
use toml::{toml, Table};

pub fn get_config() -> Table {
	toml! {
		[backup]
		paths = ["/Users", "/Applications", "/Library"]
		exclude = ["cache"]
		exclude_tags = [["CACHEDIR.TAG", "keep-tag"]]
		name = NAME
	}
}
