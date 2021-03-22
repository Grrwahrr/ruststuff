use std::fs::{self, File};
use std::io;
use md5::{Md5, Digest};
use std::path::Path;

use image::GenericImageView;
use regex::Regex;

use crate::app::utils::get_extension_from_filename;
use crate::app::utils::get_stem_from_filename;
use crate::app::utils::weak_random_base62_string;

const GALLERY_PATH: &str = "data/gallery";
const DEFAULT_PICTURE_PATH: &str = "data/gallery/not_found.png";

#[derive(Debug, Serialize)]
pub struct UploadedImage {
	guid: String,
	ext: String,
	src: String,
	hash: String,
	x: u32,
	y: u32,
}


/// Generate a new file name, check if the path is unused, return full local path
pub fn generate_upload_file_name(uploaded_name: &str) -> Result<String, String> {
	for _ in 0..25 {
		// Extract the file extension
		let extension = match get_extension_from_filename(uploaded_name) {
			Some(ext) => ext,
			_ => return Err(String::from("Could not get extension from filename")),
		};

		// Generate some random bits
		let name = weak_random_base62_string(15);

		// Put together the local path
		let path_local = format!("{}/original/{}.{}", GALLERY_PATH, name, extension);

		// Make sure the file does not yet exist
		if !Path::new(&path_local).exists() {
			return Ok(path_local);
		}
	}

	Err(String::from("All file names collide"))
}

/// Once an upload finishes, we will take a list of all uploaded files and store references in the database
pub fn finish_file_upload(local_files: &Vec<String>, db: &mysql::Pool) -> Vec<UploadedImage> {
	let mut result = vec![];
	for path in local_files {
		match uploaded_file_get_info(path) {
			Ok(image_info) => {
				// Store this info in the database
				add_image_to_gallery(&image_info, db);

				// Attach to result
				result.push(image_info);
			}
			_ => {}
		}
	}

	result
}

/// Open the file from disk and extract some info
fn uploaded_file_get_info(local_path: &str) -> Result<UploadedImage, String> {
	// Extract the file extension
	let extension = match get_extension_from_filename(local_path) {
		Some(tmp) => tmp,
		_ => return Err(String::from("Cannot get image extension")),
	};

	let stem = match get_stem_from_filename(local_path) {
		Some(tmp) => tmp,
		_ => return Err(String::from("Cannot get image file stem")),
	};

	// Hash the source file
	let mut file = fs::File::open(local_path).map_err(|_| String::from("Image not found when trying to hash"))?;
	let mut hasher = Md5::new();
	let n = io::copy(&mut file, &mut hasher).map_err(|_| String::from("Image hashing error"))?;
	let hash = hasher.finalize();

	// Open the image
	match image::open(local_path) {
		Ok(img) => {
			let (x, y) = img.dimensions();
			Ok(UploadedImage {
				guid: String::from(stem),
				ext: String::from(extension),
				src: format!("/gallery/{}/w200/thumb.{}", stem, extension),
				hash: format!("{:x}", hash),
				x,
				y,
			})
		}
		_ => { Err(String::from("Cannot open image")) }
	}
}

/// Add a new image to the gallery database
fn add_image_to_gallery(image_info: &UploadedImage, db: &mysql::Pool) {
	// INSERT INTO gallery (guid, extension, sizeX, sizeY) VALUES ()
	let query = "INSERT IGNORE INTO gallery (guid, hash, extension, sizeX, sizeY) VALUES (:guid, :hash, :extension, :x, :y)";

	// Execute
	match db.prep_exec(query, params! {"guid" => &image_info.guid, "hash" => &image_info.hash, "extension" => &image_info.ext, "x" => image_info.x, "y" => image_info.y}) {
		Ok(_) => {}
		Err(err) => { println!("Error adding image to gallery: {:?}", err); }
	}
}

/// Load all the gallery images from the database
pub fn load_gallery_from_sql(db: &mysql::Pool) -> Vec<UploadedImage> {
	let query_result = match db.prep_exec("SELECT guid, extension, sizeX, sizeY FROM gallery ORDER BY uploadedAt DESC", ()) {
		Ok(tmp) => { tmp }
		_ => { return vec![]; }
	};

	let mut images = Vec::new();

	for result_row in query_result {
		let row = match result_row {
			Ok(tmp) => tmp,
			_ => continue
		};

		match from_sql(row) {
			Some(tmp) => images.push(tmp),
			_ => {}
		}
	}

	images
}

/// Turn a SQL row into an image struct
pub fn from_sql(mut row: mysql::Row) -> Option<UploadedImage> {
	Some(UploadedImage {
		guid: row.take("guid")?,
		ext: row.take("extension")?,
		src: String::from(""),
		hash: String::from(""),
		x: row.take("sizeX")?,
		y: row.take("sizeY")?,
	})
}

/// Find the file system path for the given original
pub fn gallery_find_original(path: &str) -> String {
	// Validate input
	match Regex::new(r"[A-z0-9.]+") {
		Ok(regex) => {
			if !regex.is_match(path) { return String::from(DEFAULT_PICTURE_PATH); }
		}
		_ => { return String::from(DEFAULT_PICTURE_PATH); }
	}

	// Check if this image is in the main gallery folder
	let path_local = format!("{}/{}", GALLERY_PATH, path);
	if Path::new(&path_local).exists() {
		return path_local;
	}

	// Maybe we are requesting an original file instead?
	let path_original = format!("{}/original/{}", GALLERY_PATH, path);
	if Path::new(&path_original).exists() {
		return path_original;
	}

	// Return default image
	String::from(DEFAULT_PICTURE_PATH)
}

/// Return the file system path for the requested resource
pub fn gallery_find_file(guid: &str, size: &str, tail: &str) -> String {
	// Find the extension of the requested file
	let mut extension = String::from("");
	match Regex::new(r".(?P<ext>jpg|jpeg|gif|png)$") {
		Ok(regex) => {
			for cap in regex.captures_iter(tail) {
				extension = String::from(&cap["ext"]);
			}
		}
		_ => { return String::from(DEFAULT_PICTURE_PATH); }
	}

	// Validate size input
	match Regex::new(r"[hw][0-9]+") {
		Ok(regex) => {
			if !regex.is_match(size) { return String::from(DEFAULT_PICTURE_PATH); }
		}
		_ => { return String::from(DEFAULT_PICTURE_PATH); }
	}

	// Validate guid input
	match Regex::new(r"[A-z0-9]+") {
		Ok(regex) => {
			if !regex.is_match(guid) { return String::from(DEFAULT_PICTURE_PATH); }
		}
		_ => { return String::from(DEFAULT_PICTURE_PATH); }
	}

	// Compile the resulting local path
	let path_resized = format!("{}/{}/{}.{}", GALLERY_PATH, size, guid, extension);

//  println!("Gallery path: {}", path_resized);

	// Check if the picture exists in the given size
	if Path::new(&path_resized).exists() {
		return path_resized;
	}

	// Attempt to find the original picture
	let path_original = format!("{}/original/{}.{}", GALLERY_PATH, guid, extension);

	// Can we find the original file?
	if Path::new(&path_original).exists() {
		// Try to resize it as required
		if gallery_resize_image(&path_original, &path_resized, size, &extension) {

			return path_resized;
		} else {
			return path_original;
		}
	}

	// Return default image
	String::from(DEFAULT_PICTURE_PATH)
}

/// Resize the given image according to the specified values
pub fn gallery_resize_image(path_original: &str, path_resized: &str, size: &str, extension: &str) -> bool {
	// Load the original
	match image::open(path_original) {
		Ok(img) => {
			// Convert new size to int
			let int_size = match &size[1..].parse::<u32>() {
				Ok(tmp) => { *tmp }
				_ => { 0 }
			};

			// Some size wise constraints
			if int_size <= 25 || int_size > 2000 { return false; }

			// Assume square image
			let mut new_width = int_size;
			let mut new_height = int_size;

			// Calculate the actual aspect ratio
			let aspect_ratio = img.width() as f64 / img.height() as f64;

			// What side will we scale by?
			let side = match size.chars().next() {
				Some(c) => { c }
				_ => { return false; }
			};

			// Calculate new width or height
			if side == 'h' {
				new_width = (new_width as f64 / aspect_ratio).round() as u32;
			} else if side == 'w' {
				new_height = (new_height as f64 / aspect_ratio).round() as u32;
			}

			// Make sure we do not upscale
			if new_width > img.width() || new_height > img.height() { return false; }

			// Resize it
			let scaled = img.resize_exact(new_width, new_height, image::imageops::FilterType::Lanczos3);

			// What is the format ?
			let format = match extension {
				"bmp" => { image::ImageFormat::Bmp }
				"gif" => { image::ImageFormat::Gif }
				"png" => { image::ImageFormat::Png }
				_ => { image::ImageFormat::Jpeg }
			};

			// Make sure all the folders exist
			match fs::create_dir_all(format!("{}/{}", GALLERY_PATH, size)) {
				Ok(_tmp) => {}
				_ => {}
			}

			// Store it in the given path
			match File::create(path_resized) {
				Ok(mut output) => {
					match scaled.write_to(&mut output, format) {
						Ok(_tmp) => { return true; }
						_ => { return false; }
					}
				}
				_ => { return false; }
			}
		}
		_ => { return false; }
	}
}