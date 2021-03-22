use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use std::vec::Vec;

use crate::app::config::config_get_i64;
use crate::app::utils::*;
use crate::blog::Blog;
use crate::blog::sitemap::SiteMap;
use crate::blog::types::post::{fetch_latest_posts, fetch_most_viewed_posts, PostExcerpt};

/// Cacheable items
#[derive(Clone)]
enum CacheItem {
	PinterestPosts { decay_time: u64, data: Vec<PinterestPostCompact> },
	InstagramPosts { decay_time: u64, data: Vec<InstagramPostCompact> },
	FeaturedPosts { decay_time: u64, data: Vec<PostExcerpt> },
	LatestPosts { decay_time: u64, data: Vec<PostExcerpt> },
	CachedTag { decay_time: u64, data: Vec<PostExcerpt> },
	SiteMap { data: SiteMap },
	Html { cached_at: u64, decay_time: u64, data: String },
}

pub struct Cache {
	/// Data structure for the cache
	cache: RwLock<HashMap<String, CacheItem>>,

	/// HTML cache may be reset by setting a minimum timestamp
	html_cache_min_time: AtomicU64,
}

impl Cache {
	pub fn new() -> Cache {
		Cache {
			cache: RwLock::new(HashMap::new()),
			html_cache_min_time: AtomicU64::new(0)
		}
	}

	pub fn cache_sitemap(&self, sitemap: SiteMap) {
		match self.cache.write() {
			Ok(mut write_lock) => {
				write_lock.insert(String::from("sitemap"), CacheItem::SiteMap { data: sitemap });
			}
			_ => {}
		}
	}

	pub fn cache_html(&self, key: String, html: String) {
		let unix_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
		let life_time = config_get_i64("cache_expire_html") as u64;
		//TODO: introduce cache jitter - add some random amount of seconds +(0-60 minutes)

		let cache_key = format!("html_{}", key);

		match self.cache.write() {
			Ok(mut write_lock) => {
				write_lock.insert(cache_key, CacheItem::Html { cached_at: unix_time, decay_time: (unix_time + life_time), data: html });
			}
			_ => {}
		}
	}

	/// Cache Pinterest posts
	pub fn cache_pinterest_posts(&self) {
		// Current time - without time this system wouldn't work so we may as well crash
		let unix_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
		let life_time = config_get_i64("pinterest_lifetime") as u64;

		// Return if still valid
		if self.not_yet_expired(unix_time, "pinterest_posts") { return; }

		// Nothing in the cache so fetch the latest data from the Pinterest API
		match fetch_pinterest_feed() {
			Some(pinterest_posts) => {
				// Critical section: write lock
				match self.cache.write() {
					Ok(mut write_lock) => {
						write_lock.insert(String::from("pinterest_posts"), CacheItem::PinterestPosts { decay_time: (unix_time + life_time), data: pinterest_posts });
					}
					_ => {}
				}
			}
			_ => {}
		}
	}

	/// Cache Instagram posts
	pub fn cache_instagram_posts(&self) {
		// Current time - without time this system wouldn't work so we may as well crash
		let unix_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
		let life_time = config_get_i64("instagram_lifetime") as u64;

		// Return if still valid
		if self.not_yet_expired(unix_time, "instagram_posts") { return; }

		// Nothing in the cache so fetch the latest data from the Instagram API
		match fetch_instagram_feed() {
			Some(ig_posts) => {
				match self.cache.write() {
					Ok(mut write_lock) => {
						write_lock.insert(String::from("instagram_posts"), CacheItem::InstagramPosts { decay_time: (unix_time + life_time), data: ig_posts });
					}
					_ => {}
				}
			}
			_ => {}
		}
	}

	/// Cache excerpts for the latest posts
	pub fn cache_latest_posts(&self, blog: &Blog, db: &mysql::Pool) {
		// Current time - without time this system wouldn't work so we may as well crash
		let unix_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
		let life_time = config_get_i64("latest_posts_lifetime") as u64;

		// Return if still valid
		if self.not_yet_expired(unix_time, "latest_posts") { return; }

		// Nothing in the cache so fetch the latest data from the Instagram API
		match fetch_latest_posts(db, 8) {
			Ok(tmp) => {
				let res = blog.get_post_excerpts(&tmp);

				if res.len() > 0 {
					match self.cache.write() {
						Ok(mut write_lock) => {
							write_lock.insert(String::from("latest_posts"), CacheItem::LatestPosts { decay_time: (unix_time + life_time), data: res });
						}
						_ => {}
					}
				}
			}
			_ => {}
		}
	}

	/// Cache excerpts of the posts with the most views
	pub fn cache_featured_posts(&self, blog: &Blog, db: &mysql::Pool) {
		// Current time - without time this system wouldn't work so we may as well crash
		let unix_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
		let life_time = config_get_i64("featured_posts_lifetime") as u64;

		// Return if still valid
		if self.not_yet_expired(unix_time, "featured_posts") { return; }

		// Nothing in the cache so fetch the latest data from the Instagram API
		match fetch_most_viewed_posts(db, 8) {
			Ok(tmp) => {
				let res = blog.get_post_excerpts(&tmp);

				if res.len() > 0 {
					match self.cache.write() {
						Ok(mut write_lock) => {
							write_lock.insert(String::from("featured_posts"), CacheItem::FeaturedPosts { decay_time: (unix_time + life_time), data: res });
						}
						_ => {}
					}
				}
			}
			_ => {}
		}
	}

	/// Cache excerpts for posts with a specific tag
	pub fn cache_posts_by_tag(&self, blog: &Blog, tag_key: u8, tag: &str) {
		// Make sure the string isn't empty
		if tag == "" { return; }

		// Current time - without time this system wouldn't work so we may as well crash
		let unix_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
		let life_time = config_get_i64("cached_tag_lifetime") as u64;
		let key = format!("post_by_tag_{}", tag_key);

		// Return if still valid
		if self.not_yet_expired(unix_time, &key) { return; }

		// Nothing in the cache so get the posts for this tag from the blog object and store the data in the cache
		let res = blog.get_post_excerpts_by_tag(tag, 8);

		if res.len() > 0 {
			match self.cache.write() {
				Ok(mut write_lock) => {
					write_lock.insert(key, CacheItem::CachedTag { decay_time: (unix_time + life_time), data: res });
				}
				_ => {}
			}
		}
	}

	// ------------------------------------------------------------------
	// ------------------------ HELPER FUNCTION -------------------------
	// ------------------------------------------------------------------

	/// Check if a given cache item has expired
	fn not_yet_expired(&self, unix_time: u64, key: &str) -> bool {
		match self.get(key) {
			Some(item) => {
				let tmp = match item {
					CacheItem::PinterestPosts { decay_time, data: _ } => { decay_time }
					CacheItem::InstagramPosts { decay_time, data: _ } => { decay_time }
					CacheItem::LatestPosts { decay_time, data: _ } => { decay_time }
					CacheItem::FeaturedPosts { decay_time, data: _ } => { decay_time }
					_ => { std::u64::MAX } // Default: does not expire
				};

				unix_time <= tmp
			}
			_ => { false }
		}
	}

	/// Invalidate the entire HTML cache
	pub fn reset_html_cache(&self) {
		self.html_cache_min_time.store(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(), Ordering::Relaxed);
	}

	// ------------------------------------------------------------------
	// -------------------- CACHE RETRIEVAL FUNCTION --------------------
	// ------------------------------------------------------------------

	/// Retrieve generic items from the cache
	/// This function will `lock` (read)
	#[inline(always)]
	fn get(&self, key: &str) -> Option<CacheItem> {
		match self.cache.read() {
			Ok(guard) => {
				match guard.get(key) {
					Some(tmp) => { Some((*tmp).clone()) }
					_ => { None }
				}
			}
			_ => { None }
		}
	}

	/// Retrieve the site map from the cache
	pub fn get_site_map(&self) -> Option<SiteMap> {
		match self.get("sitemap")? {
			CacheItem::SiteMap { data } => { Some(data) }
			_ => { None }
		}
	}

	/// Fetch the Pinterest posts from the cache
	pub fn get_pinterest_posts(&self) -> Option<Vec<PinterestPostCompact>> {
		match self.get("pinterest_posts")? {
			CacheItem::PinterestPosts { decay_time: _, data } => { Some(data) }
			_ => { None }
		}
	}

	/// Fetch the Instagram posts from the cache or from the Instagram API
	pub fn get_instagram_posts(&self) -> Option<Vec<InstagramPostCompact>> {
		match self.get("instagram_posts")? {
			CacheItem::InstagramPosts { decay_time: _, data } => { Some(data) }
			_ => { None }
		}
	}

	/// Fetch excerpts of the latest posts from the cache
	pub fn get_latest_posts(&self) -> Option<Vec<PostExcerpt>> {
		match self.get("latest_posts")? {
			CacheItem::LatestPosts { decay_time: _, data } => { Some(data) }
			_ => { None }
		}
	}

	/// Fetch excerpts from the featured (most viewed) posts from the cache
	pub fn get_featured_posts(&self) -> Option<Vec<PostExcerpt>> {
		match self.get("featured_posts")? {
			CacheItem::FeaturedPosts { decay_time: _, data } => { Some(data) }
			_ => { None }
		}
	}

	/// Fetch excerpts from posts with a given tag from the cache
	pub fn get_posts_by_tag(&self, tag_key: u8) -> Option<Vec<PostExcerpt>> {
		let key = format!("post_by_tag_{}", tag_key);
		match self.get(&key)? {
			CacheItem::CachedTag { decay_time: _, data } => { Some(data) }
			_ => { None }
		}
	}

	/// Retrieve some html from the cache
	pub fn get_html(&self, key: &str) -> Option<String> {
		let cache_key = format!("html_{}", key);
		match self.get(&cache_key)? {
			CacheItem::Html { cached_at, decay_time, data } => {

				// Make sure this item did not yet expire
				let unix_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
				if decay_time < unix_time || cached_at < self.html_cache_min_time.load(Ordering::Relaxed) {
					return None;
				}

				Some(data)
			}
			_ => { None }
		}
	}
}