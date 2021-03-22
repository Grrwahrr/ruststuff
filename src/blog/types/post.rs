use chrono::{NaiveDateTime, Utc};
use serde_json::Error as JsonError;

// ------------------------------
// ------------ POST ------------
// ------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Post {
	pub id: u32,
	pub author_name: String,
	pub author_home_post: u32,
	pub date_posted: u64,
	pub date_modified: u64,
	pub state: String,
	pub title: String,
	pub content: String,

	pub meta_title: String,
	pub meta_description: String,
	pub meta_keywords: Vec<String>,

	pub url_canonical: String,
	pub url_historic: Vec<String>,

	pub tags: Vec<String>,
	pub media: Vec<PostMedia>,
	pub locations: Vec<PostLocation>,
	pub related_posts: Vec<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PostMedia {
	pub class: String,
	pub source: String,
	pub title: String,
	pub caption: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PostLocation {
	pub title: String,
	pub desc: String,
	pub lat: f32,
	pub lng: f32,
	pub typ: String,
}

impl Post {
	/// Convert the blog post to an excerpt
	pub fn get_excerpt(&self) -> PostExcerpt {
		PostExcerpt {
			id: self.id,
			author: self.author_name.clone(),
			date_posted: self.date_posted,
			title: self.title.clone(),
			content: {
				let mut res = String::from("");
				for item in self.content.split("<!--more-->") {
					res = String::from(format!("{}</p>", item));
					break;
				}
				res
			},
			content_full: self.content.clone(),
			url_canonical: self.url_canonical.clone(),
			thumbnail: {
				let mut thumb = String::from("/gallery/not_found.png");
				for item in &self.media {
					if item.class == "featured" {
						thumb = item.source.clone();
						break;
					}
				}
				thumb
			},
		}
	}

	pub fn from_sql(mut row: mysql::Row) -> Option<Post> {
		Some(Post {
			id: row.take("id")?,
			author_name: row.take("author_name")?,
			author_home_post: row.take("author_home_post")?,
			date_posted: row.take::<NaiveDateTime, _>("date_posted")?.timestamp() as u64,
			date_modified: row.take::<NaiveDateTime, _>("date_modified")?.timestamp() as u64,
			state: row.take("state")?,
			title: row.take("title")?,
			content: row.take("content")?,
			meta_title: row.take("meta_title")?,
			meta_description: row.take("meta_description")?,
			meta_keywords: match serde_json::from_str(row.take::<String, _>("meta_keywords")?.as_str()) {
				Ok(tmp) => { Some(tmp)? }
				_ => { vec![] }
			},
			url_canonical: row.take("url_canonical")?,
			url_historic: match serde_json::from_str(row.take::<String, _>("url_historic")?.as_str()) {
				Ok(tmp) => { Some(tmp)? }
				_ => { vec![] }
			},
			tags: match serde_json::from_str(row.take::<String, _>("tags")?.as_str()) {
				Ok(tmp) => { Some(tmp)? }
				_ => { vec![] }
			},
			media: match serde_json::from_str(row.take::<String, _>("media")?.as_str()) {
				Ok(tmp) => { Some(tmp)? }
				_ => { vec![] }
			},
			locations: match serde_json::from_str(row.take::<String, _>("locations")?.as_str()) {
				Ok(tmp) => { Some(tmp)? }
				_ => { vec![] }
			},
			related_posts: match serde_json::from_str(row.take::<String, _>("related_posts")?.as_str()) {
				Ok(tmp) => { Some(tmp)? }
				_ => { vec![] }
			},
		})
	}

	/// This function will be called by the admin panel to create a new or edit an existing post
	pub fn update_post_data(&self, db: &mysql::Pool) -> Result<u64, String> {
		// We will need the current unix time
		let date_time = Utc::now().naive_utc();

		// The post from the admin panel actually supplies the user id in the userName field
		let author_id = match self.author_name.parse::<u32>() {
			Ok(tmp) => tmp,
			_ => 1
		};

		// Build the query
		let query = match self.id {
			0 => {
				// This is a new post
				r##"INSERT INTO posts (
                    author_id, date_posted, date_modified, state,
                    title, content, meta_title, meta_description, meta_keywords,
                    url_canonical, url_historic,
                    tags, media, locations, related_posts
                )
                VALUES (
                    :author_id, :date_posted, :date_modified, :state,
                    :title, :content, :meta_title, :meta_description, :meta_keywords,
                    :url_canonical, :url_historic,
                    :tags, :media, :locations, :related_posts
                )"##
			}
			_ => {
				// This is an update to an existing post
				r##"UPDATE posts SET date_modified=:date_modified, state=:state,
                title=:title, content=:content, meta_title=:meta_title, meta_description=:meta_description, meta_keywords=:meta_keywords,
                url_canonical=:url_canonical, url_historic=:url_historic,
                tags=:tags, media=:media, locations=:locations, related_posts=:related_posts WHERE id=:id"##
			}
		};

		// Convert some more values
		let meta_keywords = match serde_json::to_string(&self.meta_keywords) {
			Ok(tmp) => { tmp }
			_ => { String::from("[]") }
		};
		let tags = match serde_json::to_string(&self.tags) {
			Ok(tmp) => { tmp }
			_ => { String::from("[]") }
		};
		let media = match serde_json::to_string(&self.media) {
			Ok(tmp) => { tmp }
			_ => { String::from("[]") }
		};
		let locations = match serde_json::to_string(&self.locations) {
			Ok(tmp) => { tmp }
			_ => { String::from("[]") }
		};
		let historic_urls = match serde_json::to_string(&self.url_historic) {
			Ok(tmp) => { tmp }
			_ => { String::from("[]") }
		};
		let related_posts = match serde_json::to_string(&self.related_posts) {
			Ok(tmp) => { tmp }
			_ => { String::from("[]") }
		};

		// Bind params
		let params = params! {
            "id" => &self.id, "author_id" => &author_id, "date_posted" => &date_time, "date_modified" => &date_time, "state" => &self.state,
            "title" => &self.title, "content" => &self.content, "meta_title" => &self.meta_title, "meta_description" => &self.meta_description, "meta_keywords" => &meta_keywords,
            "url_canonical" => &self.url_canonical, "url_historic" => &historic_urls,
            "tags" => &tags, "media" => &media, "locations" => &locations, "related_posts" => &related_posts
        };

		// Execute
		match db.prep_exec(query, &params) {
			Ok(res) => {
				let post_id = match self.id {
					0 => { res.last_insert_id() }
					_ => { self.id as u64 }
				};
				Ok(post_id)
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
pub struct PostExcerpt {
	pub id: u32,
	pub author: String,
	pub date_posted: u64,
	pub title: String,
	pub content: String,
	pub content_full: String,
	pub url_canonical: String,
	pub thumbnail: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AdminPostExcerpt {
	pub id: u32,
	pub author: String,
	pub date_posted: u64,
	pub date_modified: u64,
	pub state: String,
	pub title: String,
	//    pub content: String,
	pub meta_title: String,
	pub meta_description: String,
	pub url_canonical: String,
	pub tags: Option<Vec<String>>,
}


// ------------------------------
// ---------- SQL LOAD ----------
// ------------------------------

/// Load all blog posts from the database system
///
/// Attempt to read the table containing blog posts
///
/// Result will be a vector of all `Post`s found
pub fn load_posts_from_sql(db: &mysql::Pool) -> Result<Vec<Post>, JsonError> {
	let query = r###"
    SELECT
        a.display_name AS author_name, a.home_post AS author_home_post,
        p.id, p.date_posted, p.date_modified, p.state, p.title, p.content,
        p.meta_title, p.meta_description, p.meta_keywords,
        p.url_canonical, p.url_historic,
        p.tags, p.media, p.locations, p.related_posts
    FROM posts p
    INNER JOIN users a ON a.id = p.author_id
    WHERE state NOT IN ('draft')
    ORDER BY id DESC
    "###;
	// We use this order so that categories are always showing the latest post first

	let posts_vec: Vec<Post> =
		db.prep_exec(query, ())
			.map(|result| {
				// In this closure we will map `QueryResult` to `Vec<Post>`
				// `QueryResult` is iterator over `MyResult<row, err>` so first call to `map`
				// will map each `MyResult` to contained `row` (no proper error handling)
				// and second call to `map` will map each `row` to `Post`
				result.map(|x| x.unwrap()).map(|row| {
					Post::from_sql(row).unwrap()
				}).collect() // Collect posts so now `QueryResult` is mapped to `Vec<Post>`
			}).unwrap(); // Unwrap `Vec<Post>`

	Ok(posts_vec)
}

/// Find the latest posts
///
/// This will use SQL to get the ids of the latest posts
pub fn fetch_latest_posts(db: &mysql::Pool, limit: u32) -> Result<Vec<u32>, JsonError> {
	let query = r###"
    SELECT
        p.id
    FROM posts p
    WHERE 1
    ORDER BY p.date_posted DESC
    LIMIT 0, :a
    "###;

	let posts_vec: Vec<u32> =
		db.prep_exec(query, params! {"a" => limit})
			.map(|result| {
				result.map(|x| x.unwrap()).map(|mut row| {
					row.take("id").unwrap()
				}).collect()
			}).unwrap();

	Ok(posts_vec)
}

/// Find the most viewed posts
///
/// This will use SQL to get the ids of the most viewed posts
pub fn fetch_most_viewed_posts(db: &mysql::Pool, limit: u32) -> Result<Vec<u32>, JsonError> {
	let query = r###"
    SELECT post_id
    FROM post_views
    WHERE viewed_at > NOW() - INTERVAL 30 DAY
    GROUP BY post_id
    ORDER BY COUNT(*) DESC
    LIMIT 0, :a
    "###;

	let posts_vec: Vec<u32> =
		db.prep_exec(query, params! {"a" => limit})
			.map(|result| {
				result.map(|x| x.unwrap()).map(|mut row| {
					row.take("post_id").unwrap()
				}).collect()
			}).unwrap();

	Ok(posts_vec)
}

/// Find posts using the given search string
///
/// This will use SQL to get the ids of the most viewed posts
pub fn fetch_posts_by_search_string(db: &mysql::Pool, search_string: &str) -> Result<Vec<u32>, JsonError> {
	let words = search_string.split(" ");
	let mut count = 0;
	let mut title = String::from("");
	let mut content = String::from("");
	let mut params: Vec<String> = Vec::new();

	for word in words {
		// Skip if there are too many words
		if count >= 10 { break; }
		count += 1;

		// Add to a list of params
		params.push(format!("%{}%", word));

		if title == "" {
			title = format!("title LIKE ?");
			content = format!("content LIKE ?");
		} else {
			title = format!("{} AND title LIKE ?", title);
			content = format!("{} AND content LIKE ?", content);
		}
	}

	// Duplicate params
	let params_copy = params.clone();
	params.extend_from_slice(&params_copy);

	// Build the query
	let query = format!("SELECT id FROM posts WHERE ({}) OR ({}) ORDER BY id DESC ", title, content);
	//TODO make sure there is an INDEX on content, title

//  println!("Query: {} Params: {:?}", query, params);

	let posts_vec: Vec<u32> =
		db.prep_exec(query, params)
			.map(|result| {
				result.map(|x| x.unwrap()).map(|mut row| {
					row.take("id").unwrap()
				}).collect()
			}).unwrap();

	Ok(posts_vec)
}

/// Insert a post view into the table
pub fn log_post_views(db: &mysql::Pool, views: &Vec<(u32, u64, String, String, String)>) {
	// (post_id, viewed_at, remote_ip, user_agent, referer)
	for mut stmt in db.prepare(r"INSERT INTO post_views (post_id, viewed_at, remote_ip, user_agent, referer) VALUES (:id, :time, :remote, :agent, :referer)").into_iter() {
		for v in views.iter() {
			match stmt.execute(params! {"id" => v.0, "time" => NaiveDateTime::from_timestamp(v.1 as i64, 0), "remote" => &v.2, "agent" => &v.3, "referer" => &v.4}) {
				Ok(_res) => {}
				_ => {}
			}
		}
	}
}


// ------------------------------
// ---------- SQL ADMIN ---------
// ------------------------------

/// Admin function that returns a list of posts, including drafts
pub fn admin_fetch_post_list(db: &mysql::Pool) -> Option<Vec<AdminPostExcerpt>> {
	let query = r###"
    SELECT
        p.id, p.date_posted, p.date_modified, p.state, p.title, p.content, p.meta_title, p.meta_description, p.url_canonical, p.tags, a.display_name AS authorName
    FROM posts p
    INNER JOIN users a ON a.id = p.author_id
    ORDER BY id DESC
    "###;

	let query_result = match db.prep_exec(query, ()) {
		Ok(tmp) => { tmp }
		_ => { return None; }
	};

	let mut posts = Vec::new();

	for result_row in query_result {
		let mut row = match result_row {
			Ok(tmp) => { tmp }
			_ => { continue; }
		};

		posts.push(AdminPostExcerpt {
			id: row.take("id")?,
			author: row.take("authorName")?,
			date_posted: row.take::<NaiveDateTime, _>("date_posted")?.timestamp() as u64,
			date_modified: row.take::<NaiveDateTime, _>("date_modified")?.timestamp() as u64,
			state: row.take("state")?,
			title: row.take("title")?,
//          content: row.take("content").?,
			meta_title: row.take("meta_title")?,
			meta_description: row.take("meta_description")?,
			url_canonical: row.take("url_canonical")?,
			tags: match serde_json::from_str(row.take::<String, _>("tags")?.as_str()) {
				Ok(tmp) => { tmp }
				_ => { None }
			},
		});
	}

	Some(posts)
}

/// Admin function that returns the given post by its id
pub fn admin_fetch_post(db: &mysql::Pool, id: u32) -> Option<Post> {
	let query = r###"
    SELECT
        a.display_name AS author_name, a.home_post AS author_home_post,
        p.id, p.date_posted, p.date_modified, p.state, p.title, p.content,
        p.meta_title, p.meta_description, p.meta_keywords,
        p.url_canonical, p.url_historic,
        p.tags, p.media, p.locations, p.related_posts
    FROM posts p
    INNER JOIN users a ON a.id = p.author_id
    WHERE p.id = :a
    "###;

	let query_result = match db.prep_exec(query, params! {"a" => id}) {
		Ok(tmp) => { tmp }
		_ => { return None; }
	};

	for result_row in query_result {
		let row = match result_row {
			Ok(tmp) => { tmp }
			_ => { continue; }
		};

		return Post::from_sql(row);
	}

	None
}
///// Find posts using the given tag
/////
///// This will use SQL to get the ids of the most viewed posts
//pub fn fetch_posts_by_tag(db: &mysql::Pool, tag: &str) -> Result<Vec<u32>, Error> {
//    let query = r###"
//    SELECT id
//    FROM posts
//    WHERE JSON_CONTAINS(tags, :a)
//    ORDER BY id DESC
//    "###;
//
//    let posts_vec: Vec<u32> =
//        db.prep_exec(query, params! {"a" => format!("\"{}\"", tag)})
//            .map(|result| {
//                result.map(|x| x.unwrap()).map(|mut row| {
//                    row.take("id").unwrap()
//                }).collect()
//            }).unwrap();
//
//    Ok(posts_vec)
//}

//pub fn fetch_posts_by_tag(db: &mysql::Pool, tag: &str, limit: u32, offset: u32) -> Result<(Vec<u32>, u32), Error> {
//    let query = r###"
//    SELECT id
//    FROM posts
//    WHERE JSON_CONTAINS(tags, :a)"###;
//
//    let suffix = r###"
//    ORDER BY id DESC
//    LIMIT :b, :c
//    "###;
//
//    let posts_vec: Vec<u32> =
//        db.prep_exec(format!("{}{}", query, suffix), params! {"a" => format!("\"{}\"", tag), "b" => offset, "c" => limit})
//            .map(|result| {
//                result.map(|x| x.unwrap()).map(|mut row| {
//                    row.take("id").unwrap()
//                }).collect()
//            }).unwrap();
//
//    // Check if we need to query the total amount of rows
//    let mut total_posts = offset + posts_vec.len();
//    if total_posts == (offset + limit) {
//        println!("Running COUNT query");
//        for row in db.prep_exec(query.replace("SELECT id", "SELECT COUNT(*) AS count"), params! {"a" => format!("\"{}\"", tag)}).unwrap() {
//            total_posts = mysql::from_row(row.unwrap());
//        }
//    }
//    println!("TOTAL COUNT IS {}", total_posts);
//
//    Ok((posts_vec,total_posts))
//}