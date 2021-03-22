use std::ffi::OsStr;
use std::path::Path;
use std::str;

use curl::easy::Easy;
use rand::distributions::Alphanumeric;
use rand::Rng;

use crate::app::config::config_get_string;

// ------------------------------
// ---------- Helpers -----------
// ------------------------------

/// A weak random function to generate an alphanumeric string
pub fn weak_random_base62_string(len: usize) -> String {
	rand::thread_rng().sample_iter(&Alphanumeric).take(len).collect()
}

/// Extract the extension from the given file name
pub fn get_extension_from_filename(filename: &str) -> Option<&str> {
	Path::new(filename).extension().and_then(OsStr::to_str)
}

/// Extract the stem from the given file name
pub fn get_stem_from_filename(filename: &str) -> Option<&str> {
	Path::new(filename).file_stem().and_then(OsStr::to_str)
}


// ------------------------------
// ------------ CURL ------------
// ------------------------------

/// A function to curl some URL
fn curl_fetch(url: &str) -> Option<String> {
	let mut dst = Vec::new();
	{
		let mut easy = Easy::new();

		match easy.url(url) {
			Ok(()) => {}
			_ => { return None; }
		}

		let mut transfer = easy.transfer();

		match transfer.write_function(|data| {
			dst.extend_from_slice(data);
			Ok(data.len())
		}) {
			Ok(()) => {}
			_ => { return None; }
		}

		match transfer.perform() {
			Ok(()) => {}
			_ => { return None; }
		}
	}

	match String::from_utf8(dst) {
		Ok(str) => { return Some(String::from(str)); }
		_ => {}
	}

	None
}

//fn curl_post(url: &str) -> Option<String> {
//    let mut data = "this is the body".as_bytes();
//    let mut easy = Easy::new();
//
//    match easy.url(url) {
//        Ok(()) => {}
//        _ => { return None; }
//    }
//
//    match easy.post(true) {
//        Ok(()) => {}
//        _ => { return None; }
//    }
//
//    match easy.post_field_size(data.len() as u64) {
//        Ok(()) => {}
//        _ => { return None; }
//    }
//
//    let mut transfer = easy.transfer();
//
////    transfer.read_function(|buf| {
////        Ok(data.read(buf).unwrap_or(0))
////    }).unwrap();
//
//    match transfer.perform() {
//        Ok(()) => {}
//        _ => { return None; }
//    }
//
//    None
//}


// ------------------------------
// --------- INSTAGRAM ----------
// ------------------------------

#[derive(Serialize, Deserialize)]
struct InstagramFeedResult {
	data: Vec<InstagramPost>,
}

#[derive(Serialize, Deserialize)]
struct InstagramPost {
	id: String,
	media_url: String,
	permalink: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstagramPostCompact {
	link: String,
	img_src: String,
	location: String,
	likes: u32,
	comments: u32,
}

/// Uses cURL to retrieve the latest posts from the Instagram API
///
/// Use the config to set user id and api secrets
pub fn fetch_instagram_feed() -> Option<Vec<InstagramPostCompact>> {
	let token = config_get_string("instagram_token");
	let url = config_get_string("instagram_url");
	let mut req_result: Option<Vec<InstagramPost>> = None;

	match curl_fetch(url.replace("%TOKEN%", token.as_str()).as_str()) {
		Some(json_data) => {
			let tmp: Result<InstagramFeedResult, serde_json::Error> = serde_json::from_str(json_data.as_str());

			match tmp { // Could make this one line with experimental feature type_ascription
				Ok(val) => {
					req_result = Some(val.data);
				}
				Err(err) => {
					println!("Error decoding Instagram data: {:?}", err)
				}
			}
		}
		_ => {}
	}

	// Compact the data as we do not care about most of the structure
	match req_result {
		Some(vec_posts) => {
			let mut vec_result: Vec<InstagramPostCompact> = Vec::new();

			for post in vec_posts {
				vec_result.push(InstagramPostCompact {
					link: post.permalink,
					img_src: post.media_url,
					location: String::from(""),
					likes: 0,
					comments: 0,
				});
			}

			return Some(vec_result);
		}
		_ => {}
	}

	None
}


// ------------------------------
// --------- PINTEREST ----------
// ------------------------------

#[derive(Serialize, Deserialize)]
struct PinterestFeedResult {
	data: Vec<PinterestPost>,
}

#[derive(Serialize, Deserialize)]
struct PinterestPost {
	id: String,
	note: String,
	image: PinterestPostImageData,
}

#[derive(Serialize, Deserialize)]
struct PinterestPostImageData {
	original: PinterestPostImage,
}

#[derive(Serialize, Deserialize)]
struct PinterestPostImage {
	url: String,
	width: u32,
	height: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PinterestPostCompact {
	id: String,
	note: String,
	img_src: String,
}


/// Uses cURL to retrieve the latest posts from the Pinterest API
///
/// Use the config file to setup API URL and TOKEN
pub fn fetch_pinterest_feed() -> Option<Vec<PinterestPostCompact>> {
	let token = config_get_string("pinterest_token");
	let url = config_get_string("pinterest_url");
	let mut req_result: Option<Vec<PinterestPost>> = None;

	match curl_fetch(url.replace("%TOKEN%", token.as_str()).as_str()) {
		Some(json_data) => {
			//println!("pinterest debug {}", json_data);

			let tmp: Result<PinterestFeedResult, serde_json::Error> = serde_json::from_str(json_data.as_str());

			match tmp { // Could make this one line with experimental feature type_ascription
				Ok(val) => {
					req_result = Some(val.data);
				}
				_ => {}
			}
		}
		_ => {}
	}

	// Compact the data as we do not care about most of the structure
	match req_result {
		Some(vec_posts) => {
			let mut vec_result: Vec<PinterestPostCompact> = Vec::new();

			for post in vec_posts {
				vec_result.push(PinterestPostCompact {
					id: post.id,
					note: post.note,
					img_src: post.image.original.url,
				});
			}

			return Some(vec_result);
		}
		_ => {}
	}

	None
}