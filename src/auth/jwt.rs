use std::time::{SystemTime, UNIX_EPOCH};

use frank_jwt::{Algorithm, decode, encode, ValidationOptions};
use serde_json::Error;

use crate::app::config::config_get_string;
use crate::auth::user::User;

// We will use the HMAC algo for now, as we are the only signing and verifying party
const JWT_ALGO: Algorithm = Algorithm::HS256;


/// Authenticate the user and return a stringified `UserJWT` on success
pub fn handle_auth_request(db: &mysql::Pool, login: &String, pass: &String) -> Option<(u32, String, String)> {
	// Fetch required data from the user database
	let user = match User::get_user_from_db(db, login) {
		Some(tmp) => { tmp }
		_ => { return None; }
	};

	// Verify the users authenticity
	if !user.verify_password(pass) { return None; }

	// Create the token
	match UserJWT::create_token_for_user(&user).to_serde_value() {
		Ok(payload) => {
			let header = json!({});
			let secret = config_get_string("jwt_hmac_secret");

			match encode(header, &secret, &payload, JWT_ALGO) {
				Ok(jwt) => Some((user.id, user.display_name, jwt)),
				_ => None
			}
		}
		_ => None
	}
}


/// Attempt to decode and validate the stringified jwt given
///
/// on success, returns a `UserJWT`
pub fn jwt_decode(token: &String) -> Option<UserJWT> {
	match decode(token, &config_get_string("jwt_hmac_secret"), JWT_ALGO, &ValidationOptions::dangerous()) {
		Ok((_header, payload)) => {
			match UserJWT::from_serde_value(payload) {
				Ok(jwt) => Some(jwt),
				_ => None
			}
		}
		_ => { None }
	}
}


/// This is the Json Web Token (=JWT)
#[derive(Serialize, Deserialize)]
pub struct UserJWT {
	/// the subject - a user id
	pub sub: u32,
	/// issued at - the time the token was issued
	pub iat: u64,
	/// the display name of the user
	pub name: String,
	/// things the user can do
	pub permissions: Vec<String>,
}

impl UserJWT {
	/// Convert serde_json::Value into UserJWT
	pub fn from_serde_value(val: serde_json::Value) -> Result<UserJWT, Error> {
		let u: Result<UserJWT, Error> = serde_json::from_value(val);
		u
	}

	/// Convert UserJWT into serde_json::Value
	pub fn to_serde_value(&self) -> Result<serde_json::Value, Error> {
		serde_json::to_value(self)
	}

	/// Take the given users data and create a UserJWT object
	pub fn create_token_for_user(user: &User) -> UserJWT {
		UserJWT {
			sub: user.id,
			iat: match SystemTime::now().duration_since(UNIX_EPOCH) {
				Ok(tmp) => tmp.as_secs(),
				_ => 0
			},
			name: user.display_name.clone(),
			permissions: user.permissions.clone(),
		}
	}
}