use std::env;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use std::time::Duration;

use actix_cors::Cors;
use actix_files;
use actix_web::{App, Error, HttpRequest, HttpResponse, HttpServer, middleware, web};
use mysql;
use rustls::{NoClientAuth, ServerConfig};
use rustls::internal::pemfile::{certs, pkcs8_private_keys};
use tera::Tera;
use tokio::{task, time};

use crate::app::config::{config_get_i64, config_get_string, config_load_from_file};
use crate::blog::Blog;

pub mod config;
pub mod utils;


lazy_static! {
	static ref BLOG: Arc<Blog> = Arc::new(Blog::new());
}


// ------------------------------
// ----------- Routes -----------
// ------------------------------

/// Route: redirect http requests to https
fn forward_to_https(req: HttpRequest) -> HttpResponse {
	let mut target = format!("https://{}", self::config::config_get_string("fqdn"));

	if req.path().len() > 0 {
		target = format!("{}{}", &target, req.path());
	}

	if req.query_string().len() > 0 {
		target = format!("{}?{}", &target, req.query_string());
	}

	HttpResponse::Found()
		.header("LOCATION", target.as_str())
		.finish()
}

/// Route: robots.txt
fn robots() -> HttpResponse {
	HttpResponse::Ok().content_type("text/plain; charset=utf-8").body(
		format!("Sitemap: https://{}/sitemap.xml\nUser-agent: *\nDisallow: /admin", self::config::config_get_string("fqdn"))
	)
}

/// Route: favicon
pub async fn favicon() -> Result<actix_files::NamedFile, Error> {
	Ok(actix_files::NamedFile::open("./data/static/favicon.ico")?)
}


/// This function will setup the blog
/// Load all blog posts
/// And start the server
pub async fn start_https_server() -> std::io::Result<()> {
	// Load the config
	config_load_from_file().unwrap();

	// Address we will bind to
	let host_https = format!("{}:{}", config_get_string("server_host"), config_get_i64("server_ssl_port"));

	// Directories for static and template files
	let dir_static = config_get_string("server_dir_static");
	let path = env::current_dir().unwrap();
	let dir_templates = format!("{}/{}/**/*", path.to_string_lossy(), config_get_string("server_dir_templates"));

	// Setup database and connection pool
	let pool_mysql = Arc::new(mysql::Pool::new_manual(3, 10, config_get_string("server_database")).unwrap());

	// Start up the blog
	match BLOG.startup(&pool_mysql.clone()) {
		Err(err) => {
			println!("Error while setting up the blog: {}", err);
			return Err(err);
		}
		_ => {}
	}

	// Create a maintenance task
	let db_copy = pool_mysql.clone();
	let _join_handle = task::spawn(async move {
		let mut interval = time::interval(Duration::from_millis(self::config::config_get_i64("maintenance_interval") as u64));

		loop {
			interval.tick().await;
			BLOG.maintenance_task(&db_copy);
		}
	});

//    let _join_handle = thread::spawn(move || {
//        // https://tokio.rs/docs/going-deeper/timers/#running-code-on-an-interval
//        let task = Interval::new(Instant::now(), Duration::from_millis(self::config::config_get_i64("maintenance_interval") as u64))
//            .for_each(move | _instant| {
//                BLOG.maintenance_task(&db_copy);
//                Ok(())
//            })
//            .map_err(|e| panic!("interval errored; err={:?}", e));
//
//        tokio::run(task);
//    });

	// Load SSL keys
	let mut config = ServerConfig::new(NoClientAuth::new());
	let cert_file = &mut BufReader::new(File::open(config_get_string("server_ssl_crt")).unwrap());
	let key_file = &mut BufReader::new(File::open(config_get_string("server_ssl_key")).unwrap());
	let cert_chain = certs(cert_file).unwrap();
	let mut keys = pkcs8_private_keys(key_file).unwrap();
	config.set_single_cert(cert_chain, keys.remove(0)).unwrap();

	// Setup tera templates
	let tera_arc = Arc::new(Tera::new(&dir_templates).unwrap());

	// Initialize and start the threads for the https server
	HttpServer::new(move || App::new()
		.data(tera_arc.clone())
		.data(BLOG.clone())
		.data(pool_mysql.clone())

		// JSON configuration: size limit of 4mb
		.data(web::JsonConfig::default().limit(4194304))

		// JSON configuration: size limit of 16mb for editing posts
		.data(web::Json::<super::blog::types::post::Post>::configure(|cfg| cfg.limit(16777216)))

		// CORS policy
		.wrap(
			Cors::new().max_age(3600).finish()
		)
		.wrap(middleware::Logger::default())
		.wrap(middleware::Compress::default())

		// STATIC resources
		.service(actix_files::Files::new("/static", dir_static.clone()))

		// CATEGORY & SEARCH
		.service(web::resource("/tag/{name:.*}").route(web::get().to(crate::blog::routes::list_by_tag)))
		.service(web::resource("/search").route(web::get().to(crate::blog::routes::list_by_search)))

		// SITEMAP & ROBOTS & favicon
		.service(web::resource("/sitemap.xml").route(web::get().to(crate::blog::routes::sitemap)))
		.service(web::resource("/feed/").route(web::get().to(crate::blog::routes::feed)))
		.service(web::resource("/robots.txt").route(web::get().to(robots)))
		.service(web::resource("/favicon.ico").route(web::get().to(favicon)))

		// COMMENTS (let's users add unapproved comments to some blog post)
		.service(web::resource("/comment").route(web::post().to(crate::blog::routes::comment)))

		// GALLERY
		.service(web::resource("/gallery/{guid}/{size}/{tail:.*}").route(web::get().to(crate::blog::routes::gallery)))
		.service(web::resource("/gallery/{tail:.*}").route(web::get().to(crate::blog::routes::gallery_direct)))

		// REDIRECT
		.service(web::resource("/fwd/{name}").route(web::get().to(crate::blog::routes::forward)))
		.service(web::resource("/ama/{id}").route(web::get().to(crate::blog::routes::forward_amazon)))

		// AUTH routes
		.service(
			web::scope("/auth")
				.service(web::resource("/check").route(web::get().to(crate::auth::auth_check)))
				.service(web::resource("/login").route(web::post().to(crate::auth::auth_login)))
				.service(web::resource("/logout").route(web::get().to(crate::auth::auth_logout)))
		)

		// ADMIN routes
		.service(
			web::scope("/admin")
				.service(web::resource("/dashboard").route(web::get().to(crate::blog::routes_admin::dashboard)))
				.service(web::resource("/get_posts").route(web::get().to(crate::blog::routes_admin::get_posts)))
				.service(web::resource("/get_post").route(web::get().to(crate::blog::routes_admin::get_post)))
				.service(web::resource("/get_tags").route(web::get().to(crate::blog::routes_admin::get_tags)))
				.service(web::resource("/get_tag").route(web::get().to(crate::blog::routes_admin::get_tag)))
				.service(web::resource("/get_comments").route(web::get().to(crate::blog::routes_admin::get_comments)))
				.service(web::resource("/get_comment").route(web::get().to(crate::blog::routes_admin::get_comment)))
				.service(web::resource("/get_menus").route(web::get().to(crate::blog::routes_admin::get_menus)))
				.service(web::resource("/get_snippets").route(web::get().to(crate::blog::routes_admin::get_snippets)))
				.service(web::resource("/get_redirects").route(web::get().to(crate::blog::routes_admin::get_redirects)))
				.service(web::resource("/get_gallery").route(web::get().to(crate::blog::routes_admin::get_gallery)))
				.service(web::resource("/reload_data").route(web::get().to(crate::blog::routes_admin::reload_data)))

				.service(web::resource("/set_post").route(web::post().to(crate::blog::routes_admin::set_post)))
				.service(web::resource("/set_tag").route(web::post().to(crate::blog::routes_admin::set_tag)))
				.service(web::resource("/set_comment").route(web::post().to(crate::blog::routes_admin::set_comment)))
				.service(web::resource("/set_menu").route(web::post().to(crate::blog::routes_admin::set_menu)))
				.service(web::resource("/set_snippet").route(web::post().to(crate::blog::routes_admin::set_snippet)))
				.service(web::resource("/set_redirect").route(web::post().to(crate::blog::routes_admin::set_redirect)))
				.service(web::resource("/gallery/upload").route(web::post().to(crate::blog::routes_admin::gallery_upload)))
				.service(web::resource("/preview_post").route(web::post().to(crate::blog::routes_admin::preview_post)))

				.default_service(web::route().to(crate::blog::routes_admin::index))
		)

		// REACT ADMIN PANEL
		.service(
			web::scope("/ndadmin")
				.service(actix_files::Files::new("/static", "./data/admin/static").index_file("index.html"))
				//TODO: favicon.ico

				.default_service(web::route().to(crate::blog::routes_admin::index2))
		)

		// CATCH ALL | SEO fallback
		.service(web::resource("{tail:.*}").route(web::get().to(crate::blog::routes::index)))

		// Just in case the CATCH ALL didn't pick something up?
		.default_service(web::route().to(crate::blog::routes::index))
	)
		.bind_rustls(host_https.clone(), config)
		.expect(format!("Can not bind to '{}'", host_https).as_ref())
		.shutdown_timeout(60)
		.keep_alive(5)
		.run()
		.await
}

/// This server will forward all http requests to the https server
pub async fn start_http_server() -> std::io::Result<()> {
	let host_http = format!("{}:{}", config_get_string("server_host"), config_get_i64("server_port"));

    // Start the http server that forwards all requests to https
    HttpServer::new(move || App::new()
        .service(web::resource("{tail:.*}").to(forward_to_https))
    )
        .bind(host_http.clone()).expect(format!("Can not bind to '{}'", host_http).as_ref())
        .shutdown_timeout(60)    // <- Set shutdown timeout to 60 seconds
        .run()
        .await
}