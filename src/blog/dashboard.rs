#[derive(Debug, Serialize)]
pub struct DashboardPerformance {
	views_by_day: Vec<DashboardViewsByDay>,
	views_by_post: Vec<DashboardViewsByPost>,
	comments_total: u32,
	comments_new: u32,
	posts_total: u32,
	posts_unpublished: u32,
}

#[derive(Debug, Serialize)]
pub struct DashboardViewsByDay {
	date: String,
	count: u32,
}

#[derive(Debug, Serialize)]
pub struct DashboardViewsByPost {
	post_id: u32,
	last_14: u32,
	last_7: u32,
	title: String,
}

impl DashboardViewsByDay {
	pub fn from_sql(mut row: mysql::Row) -> Option<DashboardViewsByDay> {
		Some(DashboardViewsByDay {
			date: row.take("date")?,
			count: row.take("count")?,
		})
	}
}

impl DashboardViewsByPost {
	pub fn from_sql(mut row: mysql::Row) -> Option<DashboardViewsByPost> {
		Some(DashboardViewsByPost {
			post_id: row.take("post_id")?,
			last_14: row.take("last_14")?,
			last_7: row.take("last_7")?,
			title: row.take("title")?,
		})
	}
}


/// Query some statistics from the database
pub fn dashboard_get_statistics(db: &mysql::Pool) -> DashboardPerformance {
	let query_a = r###"
        SELECT DATE_FORMAT(viewed_at, '%d.%m.%Y') AS date, COUNT(id) AS count
        FROM post_views
        WHERE viewed_at >= DATE_ADD(NOW(), INTERVAL -13 DAY)
        GROUP BY DATE_FORMAT(viewed_at, '%d.%m.%Y')
    "###;

	let mut views_by_day = Vec::new();

	match db.prep_exec(&query_a, ()) {
		Ok(query_result) => {
			for result_row in query_result {
				let row = match result_row {
					Ok(tmp) => tmp,
					_ => continue
				};

				match DashboardViewsByDay::from_sql(row) {
					Some(tmp) => views_by_day.push(tmp),
					_ => {}
				}
			}
		}
		_ => {}
	}


	let query_b = r###"
        SELECT post_id, COUNT(id) AS last_14, COUNT(IF(viewed_at>=DATE_ADD(NOW(), INTERVAL -6 DAY),1, NULL)) AS last_7,
        LEFT((SELECT title FROM posts WHERE id = post_id), 30) AS title
        FROM post_views
        WHERE viewed_at >= DATE_ADD(NOW(), INTERVAL -13 DAY)
        GROUP BY post_id
        ORDER BY COUNT(id) DESC LIMIT 0,10
    "###;

	let mut views_by_post = Vec::new();

	match db.prep_exec(&query_b, ()) {
		Ok(query_result) => {
			for result_row in query_result {
				let row = match result_row {
					Ok(tmp) => tmp,
					_ => continue
				};

				match DashboardViewsByPost::from_sql(row) {
					Some(tmp) => views_by_post.push(tmp),
					_ => {}
				}
			}
		}
		_ => {}
	}

	// The number of comments as well as the number of new (unapproved comments)
	let (comments_total, comments_new) = get_comment_counts(db);

	// The number of posts as well as the number of new (unpublished posts)
	let (posts_total, posts_unpublished) = get_post_counts(db);

	DashboardPerformance {
		views_by_day,
		views_by_post,
		comments_total,
		comments_new,
		posts_total,
		posts_unpublished,
	}
}

/// This function will return the total number of comments as well as how many comments are not yet approved
fn get_comment_counts(db: &mysql::Pool) -> (u32, u32) {
	let query = "SELECT COUNT(*) AS total, SUM(case when status='new' then 1 else 0 end) AS new FROM post_comments";
	let mut comments_total = 0u32;
	let mut comments_new = 0u32;

	match db.prep_exec(&query, ()) {
		Ok(query_result) => {
			for result_row in query_result {
				let row = match result_row {
					Ok(tmp) => tmp,
					_ => continue
				};

				comments_total = match row.get("total") {
					Some(val) => val,
					_ => 0
				};
				comments_new = match row.get("new") {
					Some(val) => val,
					_ => 0
				};
			}
		}
		_ => {}
	}

	(comments_total, comments_new)
}

/// This function will return the total numbr of posts as well as the number of unpublished posts
fn get_post_counts(db: &mysql::Pool) -> (u32, u32) {
	let query = "SELECT COUNT(*) AS total, SUM(case when state!='published' then 1 else 0 end) AS unpublished FROM posts";
	let mut posts_total = 0u32;
	let mut posts_unpublished = 0u32;

	match db.prep_exec(&query, ()) {
		Ok(query_result) => {
			for result_row in query_result {
				let row = match result_row {
					Ok(tmp) => tmp,
					_ => continue
				};

				posts_total = match row.get("total") {
					Some(val) => val,
					_ => 0
				};
				posts_unpublished = match row.get("unpublished") {
					Some(val) => val,
					_ => 0
				};
			}
		}
		_ => {}
	}

	(posts_total, posts_unpublished)
}