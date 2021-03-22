use std::borrow::Cow;
use std::sync::Arc;

use actix_web::{Error, HttpMessage, HttpRequest, HttpResponse, web};
use actix_web::cookie::Cookie;

pub mod jwt;
pub mod user;


// ------------------------------
// ---------- Request -----------
// ------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthRequestUserData {
	login: String,
	pass: String,
}


// ------------------------------
// ---------- Response ----------
// ------------------------------

#[derive(Serialize)]
pub struct AuthResponseError {
	error: String,
}

#[derive(Serialize)]
pub struct AuthResponseDefault {
	#[serde(rename = "displayName")]
	display_name: String,
	#[serde(rename = "userId")]
	user_id: u32,
}

// ------------------------------
// ---------- Helpers -----------
// ------------------------------

/// Create a cookie holding the jwt for the user
pub fn create_cookie(value: &str) -> Cookie {
	let tmp = Cow::Owned(String::from(value));

	Cookie::build("nd_user", tmp)
		//.domain("www.rust-lang.org")
		.path("/")
		.http_only(true)
		.finish()
	// do we need to set life time, domain, ... ?
}

/// Returns the JWT if present and valid
pub fn is_authenticated(req: &HttpRequest) -> Option<jwt::UserJWT> {
	// Find the JWT
	let mut jwt = String::from("");

	match req.cookie("nd_user") {
		Some(cookie) => {
			jwt = String::from(cookie.value());
		}
		_ => {}
	}

	// Validate / decode token
	jwt::jwt_decode(&jwt)
}

/// Returns true if the user is an admin
pub fn is_admin(req: &HttpRequest) -> bool {
	match is_authenticated(req) {
		Some(jwt) => {
			if jwt.permissions.contains(&String::from("admin")) { return true; }
		}
		_ => {}
	}
	false
}


// ------------------------------
// ----------- Routes -----------
// ------------------------------

/// Client calls this to check whether it is logged in or not
pub async fn auth_check(req: HttpRequest) -> Result<HttpResponse, Error> {
	match is_authenticated(&req) {
		Some(jwt) => { Ok(HttpResponse::Ok().json(AuthResponseDefault { display_name: jwt.name, user_id: jwt.sub })) }
		_ => { Ok(HttpResponse::Unauthorized().json(AuthResponseError { error: String::from("token is invalid") })) }
	}
}

/// Authenticate and send the jwt cookie
pub async fn auth_login(mysql: web::Data<Arc<mysql::Pool>>, user: web::Json<AuthRequestUserData>) -> Result<HttpResponse, Error> {
	match jwt::handle_auth_request(&mysql, &user.login, &user.pass) {
		Some((user_id, display_name, jwt)) => {
			let cookie = create_cookie(&jwt);

			Ok(HttpResponse::Ok().cookie(cookie).json(AuthResponseDefault { display_name, user_id }))
		}
		_ => {
			Ok(HttpResponse::InternalServerError().json(AuthResponseError { error: String::from("invalid login") }))
		}
	}
}

/// Delete the jwt cookie
pub async fn auth_logout() -> Result<HttpResponse, Error> {
	let cookie = create_cookie("");

	Ok(HttpResponse::Ok().del_cookie(&cookie).json(AuthResponseDefault { display_name: String::from(""), user_id: 0 }))
}