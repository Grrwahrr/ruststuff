use std::sync::Arc;

use actix_files;
use actix_web::{Error, http, HttpRequest, HttpResponse, web};

use crate::blog::Blog;

// ------------------------------
// -------- FORMS & STUFF -------
// ------------------------------

#[derive(Deserialize)]
pub struct QuerySearch {
	q: String,
	p: Option<u32>,
}

#[derive(Deserialize)]
pub struct QueryPage {
	p: Option<u32>,
}

#[derive(Deserialize)]
pub struct GalleryRequest {
	guid: String,
	size: String,
	tail: String,
}

#[derive(Deserialize)]
pub struct Comment {
	post: u32,
	parent: u32,
	author: String,
	email: String,
	text: String,
	nd: String,
}

#[derive(Serialize)]
struct CommentResult {
	id: u64,
	error: String,
}

// ------------------------------
// ----------- Routes -----------
// ------------------------------


/// Route: index & seo fallback
pub async fn index(req: HttpRequest, blog: web::Data<Arc<Blog>>, tera: web::Data<Arc<tera::Tera>>, path: web::Path<String>) -> Result<HttpResponse, Error> {
	let mut seo_url = path.into_inner();

	// Remove trailing '/'
	match seo_url.chars().last() {
		Some(chr) => { if chr == '/' { seo_url.pop(); } }
		_ => {}
	}

	//DEBUG: println!("Catch all: {}", seo_url);

	// Need some additional info for statistics
	let referer = match req.headers().get("referer") {
		Some(header_val) => {
			match header_val.to_str() {
				Ok(tmp) => String::from(tmp),
				_ => String::from("")
			}
		}
		_ => String::from("")
	};
	let user_agent = match req.headers().get("user-agent") {
		Some(header_val) => {
			match header_val.to_str() {
				Ok(tmp) => String::from(tmp),
				_ => String::from("")
			}
		}
		_ => String::from("")
	};
	let remote_ip = match req.connection_info().remote() {
		Some(tmp) => String::from(tmp),
		_ => String::from("")
	};
//    println!("Remote: {}, Agent: {}, Referer: {}", &remote_ip, &user_agent, &referer);

	let mut content = String::from("");

	// Some path was specified - check our SEO urls
	if seo_url.len() > 0 {
		match blog.get_html_post(seo_url.as_str(), remote_ip, user_agent, referer, &tera) {
			Some(html) => { content = html; }
			_ => {}
		}
	}
	// If empty, this is the index route
	else {
		match blog.get_html_base(&tera, "index.html") {
			Ok(html) => { content = html; }
			Err(err) => { content = err; }
		}
	}

	// That's a 404 fall through
	if content == "" {
		match blog.get_html_base(&tera, "error_404.html") {
			Ok(html) => { content = html; }
			Err(err) => { content = err; }
		}
	}

	if content != "" {
		Ok(HttpResponse::Ok().content_type("text/html").body(content))
	} else {
		Ok(HttpResponse::InternalServerError().content_type("text/html").body(format!("Internal Server Error")))
	}
}

/// Route: tag / category
pub async fn list_by_tag(blog: web::Data<Arc<Blog>>, tera: web::Data<Arc<tera::Tera>>, mysql: web::Data<Arc<mysql::Pool>>, path: web::Path<String>, page: web::Query<QueryPage>) -> Result<HttpResponse, Error> {
	let page = match page.p {
		Some(tmp) => {
			if tmp > 0 { tmp - 1 } else { 0 }
		}
		_ => 0
	};

	match blog.get_html_tag(&mysql, &tera, path.replace("/", ""), page) {
		Ok(html) => { Ok(HttpResponse::Ok().content_type("text/html").body(html)) }
		Err(err) => { Ok(HttpResponse::InternalServerError().content_type("text/html").body(err)) }
	}
}

/// Route: search
pub async fn list_by_search(blog: web::Data<Arc<Blog>>, tera: web::Data<Arc<tera::Tera>>, mysql: web::Data<Arc<mysql::Pool>>, search: web::Query<QuerySearch>) -> Result<HttpResponse, Error> {
	let page = match search.p {
		Some(tmp) => {
			if tmp > 0 { tmp - 1 } else { 0 }
		}
		_ => 0
	};

	match blog.get_html_search(&mysql, &tera,search.q.clone(), page) {
		Ok(html) => { Ok(HttpResponse::Ok().content_type("text/html").body(html)) }
		Err(err) => { Ok(HttpResponse::InternalServerError().content_type("text/html").body(err)) }
	}
}

/// Route: sitemap.xml
pub async fn sitemap(blog: web::Data<Arc<Blog>>, tera: web::Data<Arc<tera::Tera>>) -> Result<HttpResponse, Error> {
	match blog.get_html_site_map(&tera) {
		Ok(html) => { Ok(HttpResponse::Ok().content_type("application/xml").body(html)) }
		Err(err) => { Ok(HttpResponse::InternalServerError().content_type("text/html").body(err)) }
	}
}

/// Route: feed.rss
pub async fn feed(blog: web::Data<Arc<Blog>>, tera: web::Data<Arc<tera::Tera>>) -> Result<HttpResponse, Error> {
	match blog.get_html_rss_feed(&tera) {
		Ok(html) => { Ok(HttpResponse::Ok().content_type("application/xml").body(html)) }
		Err(err) => { Ok(HttpResponse::InternalServerError().content_type("text/html").body(err)) }
	}
}

/// Route: gallery - image of specific size
pub async fn gallery(path: web::Path<GalleryRequest>) -> Result<actix_files::NamedFile, Error> {
	//TODO: add cache control for static pictures --> 2419200 seconds == 28 days (apparently not yet supported)
	Ok(actix_files::NamedFile::open(super::gallery::gallery_find_file(&path.guid, &path.size, &path.tail))?)
}

/// Route: gallery - original image
pub async fn gallery_direct(path: web::Path<String>) -> Result<actix_files::NamedFile, Error> {
	Ok(actix_files::NamedFile::open(super::gallery::gallery_find_original(&path.clone()))?)
}

/// Route: add an unapproved comment to some post
pub async fn comment(db: web::Data<Arc<mysql::Pool>>, comment: web::Json<Comment>) -> Result<HttpResponse, Error> {
	match super::comment::Comment::store_unapproved_comment(&db, comment.post, comment.parent, &comment.author, &comment.email, &comment.text, &comment.nd) {
		Ok(id) => { Ok(HttpResponse::Ok().json(CommentResult { id, error: String::from("") })) }
		Err(error) => { Ok(HttpResponse::InternalServerError().json(CommentResult { id: 0, error })) }
	}
}

/// Route: redirect generic
pub async fn forward(blog: web::Data<Arc<Blog>>, name: web::Path<String>, _page: web::Query<QueryPage>) -> Result<HttpResponse, Error> {
	Ok(HttpResponse::Found().header(http::header::LOCATION, blog.lookup_redirect(&name)).finish())
}

/// Route: redirect amazon
pub async fn forward_amazon(id: web::Path<String>, page: web::Query<QueryPage>) -> Result<HttpResponse, Error> {
	//TODO: detect user location using IP
	//TODO: get the right store address and affiliate id
	//TODO: redirect as required
	Ok(HttpResponse::Found().header(http::header::LOCATION, "/test").finish())
}