use toml::{toml, Table};

pub fn get() -> Table {
	toml! {
		[backup]
		paths = ["/Users", "/Applications", "/Library"]
	}
}
