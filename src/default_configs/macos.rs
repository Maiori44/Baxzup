use toml::{toml, Table};

pub fn get_config() -> Table {
	toml! {
		[backup]
		paths = ["/Users", "/Applications", "/Library"]
		exclude = ["cache"]
		exclude-tags = [["CACHEDIR.TAG", "keep-tag"]]
		name = "$hostname($m-$y).tar.xz"
	}
}
