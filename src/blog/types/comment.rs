use chrono::NaiveDateTime;
use serde_json::Error as JsonError;

use crate::app::config::config_get_string;

// ------------------------------
// ----------- COMMENT ----------
// ------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Comment {
	pub id: u32,
	pub parent_id: u32,
	pub post_id: u32,
	pub status: String,
	pub author_name: String,
	pub author_email: String,
	pub date_posted: u64,
	pub content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CommentExcerpt {
	pub id: u32,
	pub post_title: String,
	pub status: String,
	pub author_name: String,
	pub author_email: String,
	pub date_posted: u64,
	pub content: String,
}


impl Comment {
	pub fn from_sql(mut row: mysql::Row) -> Option<Comment> {
		Some(Comment {
			id: row.take("id")?,
			parent_id: row.take("parent_id")?,
			post_id: row.take("post_id")?,
			status: row.take("status")?,
			author_name: row.take("author_name")?,
			author_email: row.take("author_email")?,
			date_posted: row.take::<NaiveDateTime, _>("date_posted")?.timestamp() as u64,
			content: row.take("content")?,
		})
	}

	/// This function will be called by the admin panel to edit an existing comment
	pub fn update_comment_data(&self, db: &mysql::Pool) -> Result<u32, String> {
		// Build the query
		let query = "UPDATE post_comments SET status=:status,author_name=:author_name,author_email=:author_email,content=:content WHERE id=:id";

		// Bind params
		let params = params! {
            "id" => &self.id, "status" => &self.status, "author_name" => &self.author_name,
            "author_email" => &self.author_email, "date_posted" => &self.date_posted, "content" => &self.content
        };

		// Execute
		match db.prep_exec(query, &params) {
			Ok(_res) => {
				Ok(self.id)
			}
			Err(err) => {
				println!("Error: {:?}", err);
				Err(String::from(err.to_string()))
			}
		}
	}

	/// Create a new unapproved comment
	pub fn store_unapproved_comment(db: &mysql::Pool, post_id: u32, parent_id: u32, author: &str, email: &str, text: &str, bot_stop: &str) -> Result<u64, String> {
		// Check that the bot stop answer matches our current configuration
		let bot_block_answer = config_get_string("bot_block_solution");
		if bot_block_answer != bot_stop.to_lowercase().trim() {
			return Err(String::from("Please check your answer to the spam protection question."));
		}

		// There must be an author name
		let author_name = author.trim();
		if author_name.len() <= 0 {
			return Err(String::from("Kindly provide your name."));
		}

		// There must be a post the comment is to be attached to
		if post_id <= 0 {
			return Err(String::from("The post could not be found."));
		}

		// There must be some content for this comment
		let content = text.trim();
		if content.len() <= 0 {
			return Err(String::from("The comment can not be empty."));
		}

		// Build the query
		let query = "INSERT INTO post_comments (post_id,parent_id,status,author_name,author_email,content) VALUES(:post_id,:parent_id,:status,:author_name,:author_email,:content)";

		// Bind params
		let params = params! {
            "post_id" => &post_id, "parent_id" => &parent_id, "status" => "new",
            "author_name" => &author_name, "author_email" => &email, "content" => &content
        };

		// Execute
		match db.prep_exec(query, &params) {
			Ok(res) => {
				Ok(res.last_insert_id())
			}
			Err(err) => {
				println!("Error: {:?}", err);
				Err(String::from(err.to_string()))
			}
		}
	}
}


// ------------------------------
// ---------- SQL LOAD ----------
// ------------------------------

/// Load all approved blog comments from the database system
///
/// Attempt to read the table containing blog comments
///
/// Result will be a vector of all `Comment`s found
pub fn load_comments_from_sql(db: &mysql::Pool) -> Result<Vec<Comment>, JsonError> {
	let query = "SELECT id,parent_id,post_id,status,author_name,author_email,date_posted,content FROM post_comments WHERE status=:status";

	let comments: Vec<Comment> =
		db.prep_exec(query, params! {"status" => String::from("approved")})
			.map(|result| {
				// In this closure we will map `QueryResult` to `Vec<Comment>`
				// `QueryResult` is iterator over `MyResult<row, err>` so first call to `map`
				// will map each `MyResult` to contained `row` (no proper error handling)
				// and second call to `map` will map each `row` to `Comment`
				result.map(|x| x.unwrap()).map(|row| {
					Comment::from_sql(row).unwrap()
				}).collect() // Collect comments so now `QueryResult` is mapped to `Vec<Comment>`
			}).unwrap(); // Unwrap `Vec<Comment>`

	Ok(comments)
}


// ------------------------------
// ---------- SQL ADMIN ---------
// ------------------------------

/// Admin function that returns a list of comments, including drafts
pub fn admin_fetch_comment_list(db: &mysql::Pool) -> Option<Vec<CommentExcerpt>> {
	let query = r###"
    SELECT c.id,LEFT(p.title, 25) AS title,c.status,c.author_name,c.author_email,c.date_posted,LEFT(c.content, 50) AS content
    FROM post_comments AS c
    LEFT JOIN posts p ON p.id = c.post_id
    ORDER BY id DESC
    "###;

	let query_result = match db.prep_exec(query, ()) {
		Ok(tmp) => { tmp }
		_ => { return None; }
	};

	let mut comments = vec![];

	// Gather all comments that have extended data set in the database
	for result_row in query_result {
		let mut row = match result_row {
			Ok(tmp) => { tmp }
			_ => { continue; }
		};

		comments.push(CommentExcerpt {
			id: row.take("id")?,
			post_title: row.take("title")?,
			status: row.take("status")?,
			author_name: row.take("author_name")?,
			author_email: row.take("author_email")?,
			date_posted: row.take::<NaiveDateTime, _>("date_posted")?.timestamp() as u64,
			content: row.take("content")?,
		});
	}

	Some(comments)
}

/// Admin function that returns the given comments by its id
pub fn admin_fetch_comment(db: &mysql::Pool, id: u32) -> Option<Comment> {
	let query = r###"
    SELECT id, parent_id, post_id, status, author_name, author_email, date_posted, content
    FROM post_comments
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

		return Comment::from_sql(row);
	}

	None
}