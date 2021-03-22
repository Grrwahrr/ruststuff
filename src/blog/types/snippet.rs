use std::vec::Vec;

use regex::Regex;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Snippet {
	pub id: u16,
	pub name: String,
	pub replacement: String,
	pub variables: Vec<SnippetVariable>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SnippetVariable {
	pub name: String,
	pub default: String,
}

impl Snippet {
	/// Turns a SQL row into a snippet
	pub fn from_sql(mut row: mysql::Row) -> Option<Snippet> {
		Some(Snippet {
			id: row.take("id")?,
			name: row.take("name")?,
			replacement: row.take("replacement")?,
			variables: match serde_json::from_str(row.take::<String, _>("variables")?.as_str()) {
				Ok(tmp) => { Some(tmp)? }
				_ => { vec![] }
			},
		})
	}

	/// Takes a given tail match and creates a replacement string
	pub fn get_replacement(&self, tail: &str) -> String {
		// Start of with our replacement string
		let mut text = self.replacement.clone();

		// For every variable that exists replace it into the string
		for var in &self.variables {
			let mut var_value = var.default.clone();

			// Try to find a specific value in the tail
			match Regex::new(&format!("{}=\"(?P<capval>[^\"]+)\"", &var.name)) {
				Ok(regex) => {
					for cap in regex.captures_iter(tail) {
//                      println!("Matched in tail - var: {:?}, val: {:?}", &var.name, &cap["capval"]);
						var_value = String::from(&cap["capval"]);
					}
				}
				_ => {}
			}

			// Replace all occurrences of this variable in our text
			text = text.replace(&format!("{{{}}}", &var.name), &var_value);
		}

//      println!("Final replacement: {}", text);
		text
	}
}

/// Load all the snippets from the database
pub fn load_snippets_from_sql(db: &mysql::Pool) -> Option<Vec<Snippet>> {
	let query_result = match db.prep_exec("SELECT id, name, replacement, variables FROM snippets", ()) {
		Ok(tmp) => { tmp }
		_ => { return None; }
	};

	let mut snippets = Vec::new();

	for result_row in query_result {
		let row = match result_row {
			Ok(tmp) => { tmp }
			_ => { continue; }
		};

		match Snippet::from_sql(row) {
			Some(tmp) => { snippets.push(tmp); }
			_ => {}
		}
	}

	Some(snippets)
}

/// Create or update a snippet in the database
pub fn update_snippet_in_sql(db: &mysql::Pool, snip: &Snippet) -> u64 {
	let query = r##"
    INSERT INTO snippets (id, name, replacement, variables) VALUES
    (:id, :name, :replacement, :variables)
    ON DUPLICATE KEY UPDATE name=:name, replacement=:replacement, variables=:variables
    "##;

	let variables = match serde_json::to_string(&snip.variables) {
		Ok(tmp) => { tmp }
		_ => { String::from("") }
	};

	// Execute
	match db.prep_exec(query, params! {"name" => &snip.name, "replacement" => &snip.replacement, "variables" => &variables, "id" => snip.id}) {
		Ok(res) => {
			if snip.id > 0 { return snip.id as u64; }
			res.last_insert_id()
		}
		Err(err) => {
			println!("Error: {:?}", err);
			0
		}
	}
}