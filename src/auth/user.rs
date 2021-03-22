use scrypt::{scrypt_check, scrypt_simple, ScryptParams};

const SCRYPT_N: u8 = 10;
const SCRYPT_R: u32 = 8;
const SCRYPT_P: u32 = 1;

#[derive(Debug)]
pub struct User {
	pub id: u32,
	pub login: String,
	pass: String,
	salt: String,
	sn: u8,
	sr: u32,
	sp: u32,
	pub display_name: String,
	pub home_post: u32,
	pub permissions: Vec<String>,
}

impl User {
	/// Compare the provided password against the users password
	pub fn verify_password(&self, pass: &str) -> bool {
		match scrypt_check(pass, &self.pass) {
			Ok(_) => { return true; }
			_ => {}
		}

		false
	}

	/// Create a new user
	pub fn create_user(login: &str, pass: &str) -> Option<User> {
		// Make some salt
		let salt = crate::app::utils::weak_random_base62_string(128);

		// Setup scrypt params
		let params = match ScryptParams::new(SCRYPT_N, SCRYPT_R, SCRYPT_P) {
			Ok(tmp) => { tmp }
			_ => { return None; }
		};

		// Hash the password
		let hashed = match scrypt_simple(pass, &params) {
			Ok(tmp) => { tmp }
			_ => { return None; }
		};

		// Insert into the database and set the newly created id
		//TODO
		Some(User {
			id: 0,
			login: String::from(login),
			pass: hashed,
			salt,
			sn: SCRYPT_N,
			sr: SCRYPT_R,
			sp: SCRYPT_P,
			display_name: String::from(login),
			home_post: 0,
			permissions: vec![String::from("guest")],
		})
	}

	/// Fetch a user from the database
	pub fn get_user_from_db(db: &mysql::Pool, login: &str) -> Option<User> {
		let query = r"SELECT id,login,pass,salt,sn,sr,sp,display_name,home_post,permissions FROM users WHERE login = :a";

		let query_result = match db.prep_exec(query, params! {"a" => login}) {
			Ok(tmp) => { tmp }
			_ => { return None; }
		};

		for result_row in query_result {
			let mut row = match result_row {
				Ok(tmp) => { tmp }
				_ => { continue; }
			};

			return Some(User {
				id: row.take("id")?,
				login: row.take("login")?,
				pass: row.take("pass")?,
				salt: row.take("salt")?,
				sn: row.take("sn")?,
				sr: row.take("sr")?,
				sp: row.take("sp")?,
				display_name: row.take("display_name")?,
				home_post: row.take("home_post")?,
				permissions: match serde_json::from_str(row.take::<String, _>("permissions")?.as_str()) {
					Ok(tmp) => { Some(tmp)? }
					_ => { vec![] }
				},
			});
		}

		None
	}
}