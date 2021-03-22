use std::collections::HashMap;

use serde_json::Error as JsonError;

// ------------------------------
// ------------ TAG -------------
// ------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Tag {
	pub id: String,
	pub title: String,
	pub content: String,
	pub meta_title: String,
	pub meta_description: String,
	pub media: Vec<TagMedia>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TagMedia {
	pub class: String,
	pub source: String,
	pub title: String,
	pub caption: String,
}


impl Tag {
	pub fn from_sql(mut row: mysql::Row) -> Option<Tag> {
		Some(Tag {
			id: row.take("id")?,
			title: row.take("title")?,
			content: row.take("content")?,
			meta_title: row.take("meta_title")?,
			meta_description: row.take("meta_description")?,
			media: match serde_json::from_str(row.take::<String, _>("media")?.as_str()) {
				Ok(tmp) => { Some(tmp)? }
				_ => { vec![] }
			},
		})
	}

	/// This function will be called by the admin panel to create a tag or edit an existing tag
	pub fn update_tag_data(&self, db: &mysql::Pool) -> Result<String, String> {
		// Build the query
		let query = r##"REPLACE INTO tags (id, title, content, meta_title, meta_description, media)
            VALUES (:id, :title, :content, :meta_title, :meta_description, :media)"##;

		// Convert some more values
		let media = match serde_json::to_string(&self.media) {
			Ok(tmp) => { tmp }
			_ => { String::from("[]") }
		};

		// Bind params
		let params = params! {
            "id" => &self.id, "title" => &self.title, "content" => &self.content, "meta_title" => &self.meta_title, "meta_description" => &self.meta_description, "media" => &media
        };

		// Execute
		match db.prep_exec(query, &params) {
			Ok(_res) => {
				Ok(self.id.clone())
			}
			Err(err) => {
				println!("Error: {:?}", err);
				Err(String::from(err.to_string()))
			}
		}
	}
}


// ------------------------------
// ----------- EXCERPT ----------
// ------------------------------


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AdminTagExcerpt {
	pub id: String,
	pub title: String,
	pub content: String,
	pub meta_title: String,
	pub meta_description: String,
}


// ------------------------------
// ---------- SQL LOAD ----------
// ------------------------------

/// Load all blog tags from the database system
///
/// Attempt to read the table containing blog tags
///
/// Result will be a vector of all `Tag`s found
pub fn load_tags_from_sql(db: &mysql::Pool) -> Result<Vec<Tag>, JsonError> {
	let query = "SELECT id, title, content, meta_title, meta_description, media FROM tags";

	let tags: Vec<Tag> =
		db.prep_exec(query, ())
			.map(|result| {
				// In this closure we will map `QueryResult` to `Vec<Tag>`
				// `QueryResult` is iterator over `MyResult<row, err>` so first call to `map`
				// will map each `MyResult` to contained `row` (no proper error handling)
				// and second call to `map` will map each `row` to `Tag`
				result.map(|x| x.unwrap()).map(|row| {
					Tag::from_sql(row).unwrap()
				}).collect() // Collect tags so now `QueryResult` is mapped to `Vec<Tag>`
			}).unwrap(); // Unwrap `Vec<Tag>`

	Ok(tags)
}


// ------------------------------
// ---------- SQL ADMIN ---------
// ------------------------------

/// Admin function that returns a list of tags, including drafts
pub fn admin_fetch_tag_list(db: &mysql::Pool, in_use_tags: &Vec<String>) -> Option<Vec<AdminTagExcerpt>> {
	let query = r###"
    SELECT id, LEFT(title, 20) AS title, LEFT(content, 20) AS content, LEFT(meta_title, 20) AS meta_title, LEFT(meta_description, 20) AS meta_description
    FROM tags
    "###;

	let query_result = match db.prep_exec(query, ()) {
		Ok(tmp) => { tmp }
		_ => { return None; }
	};

	let mut tag_map = HashMap::new();

	// Gather all tags that have extended data set in the database
	for result_row in query_result {
		let mut row = match result_row {
			Ok(tmp) => { tmp }
			_ => { continue; }
		};

		let tag = AdminTagExcerpt {
			id: row.take("id")?,
			title: row.take("title")?,
			content: row.take("content")?,
			meta_title: row.take("meta_title")?,
			meta_description: row.take("meta_description")?,
		};

		tag_map.insert(tag.id.clone(), tag);
	}

	// Check all tags that are in use
	for tag_id in in_use_tags {
		let tmp = AdminTagExcerpt {
			id: tag_id.clone(),
			title: String::from(""),
			content: String::from(""),
			meta_title: String::from(""),
			meta_description: String::from(""),
		};

		if !tag_map.contains_key(&tmp.id) {
			tag_map.insert(tmp.id.clone(), tmp);
		}
	}

	// Convert to vector
	let mut tags = vec![];
	for (_key, tag) in tag_map {
		tags.push(tag);
	}

	// Sort the vector so that the tags do not bounce around
	tags.sort_by(|a, b| a.id.cmp(&b.id));

	Some(tags)
}

/// Admin function that returns the given tag by its id
pub fn admin_fetch_tag(db: &mysql::Pool, id: &str) -> Option<Tag> {
	let query = r###"
    SELECT id, title, content, meta_title, meta_description, media
    FROM tags
    WHERE id = :id
    "###;

	let query_result = match db.prep_exec(query, params! {"id" => id}) {
		Ok(tmp) => { tmp }
		_ => { return None; }
	};

	for result_row in query_result {
		let row = match result_row {
			Ok(tmp) => { tmp }
			_ => { continue; }
		};

		return Tag::from_sql(row);
	}

	None
}