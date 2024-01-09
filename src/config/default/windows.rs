use toml::{toml, Table};

pub fn get() -> Table {
	toml! {
		[backup]
		paths = ["C:\\"]
		exclude = ["C:\\Program Files", "C:\\Program Files (x86)", "C:\\Windows"]
	}
}
