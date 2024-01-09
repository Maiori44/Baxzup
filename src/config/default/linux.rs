use toml::{toml, Table};

pub fn get() -> Table {
	toml! {
		[backup]
		paths = ["/etc", "/home", "/root", "/var"]
	}
}
