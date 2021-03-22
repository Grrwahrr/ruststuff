use std::io::Write;
use std::sync::Arc;

use actix_files;
use actix_multipart::{Field, Multipart};
use actix_web::{error, Error, HttpRequest, HttpResponse, web};
use futures::StreamExt;
use tera::Context;

use crate::blog::Blog;
use crate::blog::dashboard::dashboard_get_statistics;
use crate::blog::gallery::finish_file_upload;
use crate::blog::gallery::generate_upload_file_name;

// ------------------------------
// -------- FORMS & STUFF -------
// ------------------------------

#[derive(Deserialize)]
pub struct GetPostRequest {
	id: u32,
}

#[derive(Deserialize)]
pub struct GetTagRequest {
	id: String,
}

#[derive(Deserialize)]
pub struct GetCommentRequest {
	id: u32,
}

#[derive(Deserialize)]
pub struct ReloadDataRequest {
	which: String,
}

#[derive(Serialize)]
struct SetPostResult {
	post_id: u64,
	error: String,
}

#[derive(Serialize)]
struct SetTagResult {
	tag_id: String,
	error: String,
}

#[derive(Serialize)]
struct SetCommentResult {
	comment_id: u32,
	error: String,
}

#[derive(Serialize)]
struct ReloadDataResult {
	success: bool,
	num: usize,
}


/// Route: admin index
pub async fn index() -> Result<actix_files::NamedFile, Error> {
	Ok(actix_files::NamedFile::open("./data/static/admin/index.html")?)
}

pub async fn index2() -> Result<actix_files::NamedFile, Error> {
	Ok(actix_files::NamedFile::open("./data/admin/index.html")?)
}

pub async fn preview_post(ctx: web::Json<super::context::Context>, template: web::Data<Arc<tera::Tera>>, req: HttpRequest) -> Result<HttpResponse, Error> {
	if crate::auth::is_admin(&req) {
		match template.render("post.html", &Context::from_serialize(&ctx.into_inner()).map_err(|_| error::ErrorInternalServerError("Template error"))?) {
			Ok(s) => { Ok(HttpResponse::Ok().content_type("text/html").body(s)) }
			_ => { Ok(HttpResponse::InternalServerError().content_type("text/html").body("Template problem")) }
		}
	} else {
		Ok(HttpResponse::Unauthorized().content_type("text/html").body("Unauthorized"))
	}
}

pub async fn reload_data(rld: web::Query<ReloadDataRequest>, blog: web::Data<Arc<Blog>>, mysql: web::Data<Arc<mysql::Pool>>, req: HttpRequest) -> Result<HttpResponse, Error> {
	if crate::auth::is_admin(&req) {
		let res = match rld.which.as_str() {
			"comments" => { blog.reload_comments(&mysql) }
			"html" => { blog.invalidate_html_cache() }
			"menus" => { blog.reload_menus(&mysql) }
			"posts" => { blog.reload_posts(&mysql) }
			"redirects" => { blog.reload_redirects(&mysql) }
			"tags" => { blog.reload_tags(&mysql) }
			_ => { Ok(0) }
		};

		match res {
			Err(_err) => { Ok(HttpResponse::Ok().json(ReloadDataResult { success: false, num: 0 })) }
			Ok(tmp) => { Ok(HttpResponse::Ok().json(ReloadDataResult { success: true, num: tmp })) }
		}
	} else {
		Ok(HttpResponse::Unauthorized().content_type("application/json").body("{}"))
	}
}


/// Route: admin - get a list of all posts
pub async fn get_posts(mysql: web::Data<Arc<mysql::Pool>>, req: HttpRequest) -> Result<HttpResponse, Error> {
	if crate::auth::is_admin(&req) {
		Ok(HttpResponse::Ok().json(
			super::post::admin_fetch_post_list(&mysql)
		))
	} else {
		Ok(HttpResponse::Unauthorized().content_type("application/json").body("{}"))
	}
}

/// Route: admin - get details for a specific post
pub async fn get_post(mysql: web::Data<Arc<mysql::Pool>>, post: web::Query<GetPostRequest>, req: HttpRequest) -> Result<HttpResponse, Error> {
	if crate::auth::is_admin(&req) {
		Ok(HttpResponse::Ok().json(
			super::post::admin_fetch_post(&mysql, post.id)
		))
	} else {
		Ok(HttpResponse::Unauthorized().content_type("application/json").body("{}"))
	}
}

/// Route: admin - update a specific post
pub async fn set_post(mysql: web::Data<Arc<mysql::Pool>>, post: web::Json<super::post::Post>, req: HttpRequest) -> Result<HttpResponse, Error> {
	if crate::auth::is_admin(&req) {
		let res = match post.update_post_data(&mysql) {
			Ok(post_id) => { SetPostResult { post_id, error: String::from("") } }
			Err(err) => { SetPostResult { post_id: 0, error: err } }
		};

		Ok(HttpResponse::Ok().json(res))
	} else {
		Ok(HttpResponse::Unauthorized().content_type("application/json").body("{}"))
	}
}

/// Route: admin - get a list of all tags
pub async fn get_tags(mysql: web::Data<Arc<mysql::Pool>>, blog: web::Data<Arc<Blog>>, req: HttpRequest) -> Result<HttpResponse, Error> {
	if crate::auth::is_admin(&req) {
		let in_use_tags = blog.get_all_in_use_tags();
		Ok(HttpResponse::Ok().json(
			super::tag::admin_fetch_tag_list(&mysql, &in_use_tags)
		))
	} else {
		Ok(HttpResponse::Unauthorized().content_type("application/json").body("{}"))
	}
}

/// Route: admin - get details for a specific tag
pub async fn get_tag(mysql: web::Data<Arc<mysql::Pool>>, tag: web::Query<GetTagRequest>, req: HttpRequest) -> Result<HttpResponse, Error> {
	if crate::auth::is_admin(&req) {
		Ok(HttpResponse::Ok().json(
			super::tag::admin_fetch_tag(&mysql, &tag.id)
		))
	} else {
		Ok(HttpResponse::Unauthorized().content_type("application/json").body("{}"))
	}
}

/// Route: admin - update a specific tag
pub async fn set_tag(mysql: web::Data<Arc<mysql::Pool>>, tag: web::Json<super::tag::Tag>, req: HttpRequest) -> Result<HttpResponse, Error> {
	if crate::auth::is_admin(&req) {
		let res = match tag.update_tag_data(&mysql) {
			Ok(tag_id) => { SetTagResult { tag_id, error: String::from("") } }
			Err(err) => { SetTagResult { tag_id: String::from(""), error: err } }
		};

		Ok(HttpResponse::Ok().json(res))
	} else {
		Ok(HttpResponse::Unauthorized().content_type("application/json").body("{}"))
	}
}

/// Route: admin - get a list of all comments
pub async fn get_comments(mysql: web::Data<Arc<mysql::Pool>>, req: HttpRequest) -> Result<HttpResponse, Error> {
	if crate::auth::is_admin(&req) {
		Ok(HttpResponse::Ok().json(
			super::comment::admin_fetch_comment_list(&mysql)
		))
	} else {
		Ok(HttpResponse::Unauthorized().content_type("application/json").body("{}"))
	}
}

/// Route: admin - get details for a specific comment
pub async fn get_comment(mysql: web::Data<Arc<mysql::Pool>>, comment: web::Query<GetCommentRequest>, req: HttpRequest) -> Result<HttpResponse, Error> {
	if crate::auth::is_admin(&req) {
		Ok(HttpResponse::Ok().json(
			super::comment::admin_fetch_comment(&mysql, comment.id)
		))
	} else {
		Ok(HttpResponse::Unauthorized().content_type("application/json").body("{}"))
	}
}

/// Route: admin - update a specific comment
pub async fn set_comment(mysql: web::Data<Arc<mysql::Pool>>, comment: web::Json<super::comment::Comment>, req: HttpRequest) -> Result<HttpResponse, Error> {
	if crate::auth::is_admin(&req) {
		let res = match comment.update_comment_data(&mysql) {
			Ok(comment_id) => { SetCommentResult { comment_id, error: String::from("") } }
			Err(err) => { SetCommentResult { comment_id: 0, error: err } }
		};

		Ok(HttpResponse::Ok().json(res))
	} else {
		Ok(HttpResponse::Unauthorized().content_type("application/json").body("{}"))
	}
}

/// Route: admin - get details for all menus
pub async fn get_menus(mysql: web::Data<Arc<mysql::Pool>>, req: HttpRequest) -> Result<HttpResponse, Error> {
	if crate::auth::is_admin(&req) {
		Ok(HttpResponse::Ok().json(super::menu::load_menus_from_sql(&mysql)))
	} else {
		Ok(HttpResponse::Unauthorized().content_type("application/json").body("{}"))
	}
}

/// Route: admin - update a specific menu
pub async fn set_menu(mysql: web::Data<Arc<mysql::Pool>>, menu: web::Json<super::menu::Menu>, req: HttpRequest) -> Result<HttpResponse, Error> {
	if crate::auth::is_admin(&req) {
		let menu_id = super::menu::update_menu_in_sql(&mysql, &menu);
		Ok(HttpResponse::Ok().content_type("application/json").body(format!("{{\"id\":{}}}", menu_id)))
	} else {
		Ok(HttpResponse::Unauthorized().content_type("application/json").body("{}"))
	}
}

/// Route: admin - get details for all snippets
pub async fn get_snippets(mysql: web::Data<Arc<mysql::Pool>>, req: HttpRequest) -> Result<HttpResponse, Error> {
	if crate::auth::is_admin(&req) {
		Ok(HttpResponse::Ok().json(super::snippet::load_snippets_from_sql(&mysql)))
	} else {
		Ok(HttpResponse::Unauthorized().content_type("application/json").body("{}"))
	}
}

/// Route: admin - update a specific snippet
pub async fn set_snippet(mysql: web::Data<Arc<mysql::Pool>>, snippet: web::Json<super::snippet::Snippet>, req: HttpRequest) -> Result<HttpResponse, Error> {
	if crate::auth::is_admin(&req) {
		let snippet_id = super::snippet::update_snippet_in_sql(&mysql, &snippet);
		Ok(HttpResponse::Ok().content_type("application/json").body(format!("{{\"id\":{}}}", snippet_id)))
	} else {
		Ok(HttpResponse::Unauthorized().content_type("application/json").body("{}"))
	}
}

/// Route: admin - get details for all redirects
pub async fn get_redirects(mysql: web::Data<Arc<mysql::Pool>>, req: HttpRequest) -> Result<HttpResponse, Error> {
	if crate::auth::is_admin(&req) {
		Ok(HttpResponse::Ok().json(super::redirect::load_redirects_from_sql(&mysql)))
	} else {
		Ok(HttpResponse::Unauthorized().content_type("application/json").body("{}"))
	}
}

/// Route: admin - update a specific redirect
pub async fn set_redirect(mysql: web::Data<Arc<mysql::Pool>>, redirect: web::Json<super::redirect::Redirect>, req: HttpRequest) -> Result<HttpResponse, Error> {
	if crate::auth::is_admin(&req) {
		let redirect_id = super::redirect::update_redirect_in_sql(&mysql, &redirect);
		Ok(HttpResponse::Ok().content_type("application/json").body(format!("{{\"id\":{}}}", redirect_id)))
	} else {
		Ok(HttpResponse::Unauthorized().content_type("application/json").body("{}"))
	}
}

/// Route: admin - get the gallery data
pub async fn get_gallery(mysql: web::Data<Arc<mysql::Pool>>, req: HttpRequest) -> Result<HttpResponse, Error> {
	if crate::auth::is_admin(&req) {
		Ok(HttpResponse::Ok().json(super::gallery::load_gallery_from_sql(&mysql)))
	} else {
		Ok(HttpResponse::Unauthorized().content_type("application/json").body("{}"))
	}
}


/// Route: admin - get a bunch of statistics for the dashboard
pub async fn dashboard(mysql: web::Data<Arc<mysql::Pool>>, req: HttpRequest) -> Result<HttpResponse, Error> {
	if crate::auth::is_admin(&req) {
		Ok(HttpResponse::Ok().json(dashboard_get_statistics(&mysql)))
	} else {
		Ok(HttpResponse::Unauthorized().content_type("application/json").body("{}"))
	}
}

/// Route: admin - upload an image to the gallery
pub async fn gallery_upload(mut multipart: Multipart, mysql: web::Data<Arc<mysql::Pool>>, req: HttpRequest) -> Result<HttpResponse, Error> {
	if !crate::auth::is_admin(&req) {
		return Err(error::ErrorUnauthorized(""));
	}

	let mut uploads = vec![];
	//TODO: fix 2 unwraps

	while let Some(item) = multipart.next().await {
		let mut field = item?;

		// The local path we want to store the uploaded file at
		let local_file_name = match prepare_upload_file_path(&field) {
			Ok(tmp_path) => tmp_path,
			Err(e) => return Err(e),
		};

		// Create the file in the local file system
		let local_file_name_clone = local_file_name.clone();
		let mut file = web::block(move || std::fs::File::create(local_file_name_clone))
			.await
			.unwrap();

		// Field in turn is stream of *Bytes* object
		while let Some(chunk) = field.next().await {
			let data = chunk.unwrap();
			// filesystem operations are blocking, we have to use threadpool
			file = web::block(move || file.write_all(&data).map(|_| file)).await?;
		}

		// Store the uploaded path in a vector
		uploads.push(local_file_name);
	}

	// Have to insert some data into the database at this point
	let result = finish_file_upload(&uploads, &mysql);

	Ok(HttpResponse::Ok().json(result))
}

/// Prepare the local path and file for the upload
fn prepare_upload_file_path(field: &Field) -> Result<String, Error> {
	// Get the content disposition
	let content_disposition = match field.content_disposition() {
		Some(tmp) => tmp,
		_ => return Err(error::ErrorInternalServerError("Could not get content disposition"))
	};

	// Find the file name specified by the user
	let input_file_name = match content_disposition.get_filename() {
		Some(filename) => filename.to_string(),
		None => return Err(error::ErrorInternalServerError("Could not retrieve the file name"))
	};

	// Get a full path for the new file we will create
	let local_file_path = match generate_upload_file_name(&input_file_name) {
		Ok(tmp) => tmp,
		Err(tmp) => return Err(error::ErrorInternalServerError(tmp))
	};

	Ok(local_file_path)
}