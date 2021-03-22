#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate mysql;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate tera;

mod app;
mod auth;
mod blog;

#[actix_rt::main]
async fn main() {
	// This is the HTTP server, all requests will be redirected to HTTPS
	actix_rt::spawn(async move {
		match app::start_http_server().await {
			Err(err) => {
				println!("HTTP server crashed: {:?}", err);
			}
			_ => {}
		}
	});

	// The HTTPS server
	app::start_https_server().await.unwrap()
}