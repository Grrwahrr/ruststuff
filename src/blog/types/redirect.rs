#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Redirect {
	pub id: u32,
	pub name: String,
	pub target: String,
}

impl Redirect {
	/// Turns a SQL row into a redirect
	pub fn from_sql(mut row: mysql::Row) -> Option<Redirect> {
		Some(Redirect {
			id: row.take("id")?,
			name: row.take("name")?,
			target: row.take("target")?,
		})
	}
}

/// Load all the redirects from the database
pub fn load_redirects_from_sql(db: &mysql::Pool) -> Option<Vec<Redirect>> {
	let query_result = match db.prep_exec("SELECT id, name, target FROM redirects", ()) {
		Ok(tmp) => { tmp }
		_ => { return None; }
	};

	let mut redirects = Vec::new();

	for result_row in query_result {
		let row = match result_row {
			Ok(tmp) => { tmp }
			_ => { continue; }
		};

		match Redirect::from_sql(row) {
			Some(tmp) => { redirects.push(tmp); }
			_ => {}
		}
	}

	Some(redirects)
}

/// Create or update a redirect in the database
pub fn update_redirect_in_sql(db: &mysql::Pool, redir: &Redirect) -> u64 {
	let query = r##"
    INSERT INTO redirects (id, name, target) VALUES
    (:id, :name, :target)
    ON DUPLICATE KEY UPDATE name=:name, target=:target
    "##;

	// Execute
	match db.prep_exec(query, params! {"name" => &redir.name, "target" => &redir.target, "id" => redir.id}) {
		Ok(res) => {
			if redir.id > 0 { return redir.id as u64; }
			res.last_insert_id()
		}
		Err(err) => {
			println!("Error: {:?}", err);
			0
		}
	}
}