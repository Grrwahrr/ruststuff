use std::error::Error;
use std::sync::RwLock;

use config::Config;

lazy_static! {
	static ref CONFIG: RwLock<Config> = RwLock::new(Config::default());
}

/// Load the configuration from a file
pub fn config_load_from_file() -> Result<(), Box<dyn Error>> {
	CONFIG.write()?.merge(config::File::with_name("config"))?;
	Ok(())
}

/// Retrieve a string type from the config
pub fn config_get_string(k: &str) -> String {
	match CONFIG.read() {
		Ok(guard) => {
			match guard.get_str(k) {
				Ok(tmp) => {
					return tmp;
				}
				_ => {}
			}
		}
		_ => {}
	}

	String::from("")
}

/// Retrieve a signed 64 bit integer from the config
pub fn config_get_i64(k: &str) -> i64 {
	match CONFIG.read() {
		Ok(guard) => {
			match guard.get_int(k) {
				Ok(tmp) => {
					return tmp;
				}
				_ => {}
			}
		}
		_ => {}
	}

	0
}