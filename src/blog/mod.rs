use std::collections::HashMap;
use std::io;
use std::sync::{Mutex, RwLock, Arc};
use std::time::{SystemTime, UNIX_EPOCH};
use std::vec::Vec;

use regex::Regex;

use crate::app::config::{config_get_i64, config_get_string};
use crate::blog::cache::Cache;
use crate::blog::context::Context;
use crate::blog::sitemap::*;
use crate::blog::types::{comment, menu, post, redirect, snippet, tag};
use crate::blog::types::comment::Comment;
use crate::blog::types::post::{Post, PostExcerpt};
use crate::blog::types::tag::Tag;
use actix_web::{error, web};

pub mod cache;
pub mod context;
pub mod types;
pub mod dashboard;
pub mod gallery;
pub mod routes;
pub mod routes_admin;
pub mod sitemap;


/// Internal messages the blog can send
pub enum BlogMessage {
	PostView { post_id: u32, viewed_at: u64, remote_ip: String, user_agent: String, referer: String }
}


/// Main blog data structure
pub struct Blog {
	posts: RwLock<HashMap<u32, Post>>,
	post_excerpts: RwLock<HashMap<u32, PostExcerpt>>,
	seo_urls: RwLock<HashMap<String, u32>>,
	seo_urls_historic: RwLock<HashMap<String, u32>>,
	comments: RwLock<HashMap<u32, Vec<Comment>>>,
	tags: RwLock<HashMap<String, Tag>>,
	tag_2_posts: RwLock<HashMap<String, Vec<u32>>>,
	menus: RwLock<HashMap<String, Vec<menu::MenuItem>>>,
	redirects: RwLock<HashMap<String, String>>,
	cache: Cache,
	messages: Mutex<Vec<BlogMessage>>,
}

impl Blog {
	/// Constructor
	pub fn new() -> Blog {
		Blog {
			posts: RwLock::new(HashMap::new()),
			post_excerpts: RwLock::new(HashMap::new()),
			seo_urls: RwLock::new(HashMap::new()),
			seo_urls_historic: RwLock::new(HashMap::new()),
			comments: RwLock::new(HashMap::new()),
			tags: RwLock::new(HashMap::new()),
			tag_2_posts: RwLock::new(HashMap::new()),
			menus: RwLock::new(HashMap::new()),
			redirects: RwLock::new(HashMap::new()),
			cache: Cache::new(),
			messages: Mutex::new(Vec::new()),
		}
	}

	/// Load the blog data from SQL
	///
	/// Cache
	///     Instagram
	///     Pinterest
	///     Featured Posts
	///     Latest Posts
	///
	/// Returns the number of blog posts that were loaded
	pub fn startup(&self, db: &mysql::Pool) -> Result<usize, io::Error> {
		// Reload blog post data
		let post_count = self.reload_posts(db)?;

		// Reload blog menus
		let menu_count = self.reload_menus(db)?;

		// Reload blog redirects
		let redirect_count = self.reload_redirects(db)?;

		// Reload blog tags
		let tag_count = self.reload_tags(db)?;

		// Reload blog comments
		let comment_count = self.reload_comments(db)?;

		// Drop a note on how much of what we have loaded
		println!("Startup found {} posts, {} tags, {} comments, {} menus, {} redirects", post_count, tag_count, comment_count, menu_count, redirect_count);

		// Cache Pinterest, Instagram, featured and latest posts
		self.cache.cache_pinterest_posts();
		self.cache.cache_instagram_posts();
		self.cache.cache_latest_posts(&self, db);
		self.cache.cache_featured_posts(&self, db);

		// We want certain tags available on the start page
		// These tags can be changed in the config
		self.cache.cache_posts_by_tag(&self, 1, config_get_string("cached_tag_1").as_str());
		self.cache.cache_posts_by_tag(&self, 2, config_get_string("cached_tag_2").as_str());
		self.cache.cache_posts_by_tag(&self, 3, config_get_string("cached_tag_3").as_str());
		self.cache.cache_posts_by_tag(&self, 4, config_get_string("cached_tag_4").as_str());
		self.cache.cache_posts_by_tag(&self, 5, config_get_string("cached_tag_5").as_str());

		Ok(post_count)
	}

	// ------------------------------------------------------------------
	// --------------------- DATA LOADING FUNCTIONS ---------------------
	// ------------------------------------------------------------------

	/// Load the blog post data from SQL
	///
	/// This function will `lock` (write)
	fn reload_posts(&self, db: &mysql::Pool) -> Result<usize, io::Error> {
		// Load all blog posts
		let blog_posts = post::load_posts_from_sql(db)?;
		let post_count = blog_posts.len();

		// Use the post data to build the sitemap
		self.reload_sitemap(&blog_posts);

		// Fetch all snippets - we will need these to do some replacing in the posts
		let snippets = match snippet::load_snippets_from_sql(db) {
			Some(tmp) => { tmp }
			_ => { vec![] }
		};

		// Create a regular expression to find snippets
		let regex = Regex::new(r"\[(?P<key>[^\s^\]]+)[\s]*(?P<tail>[^]]*)\]").unwrap();

		// CRITICAL SECTION: Load blog posts, map SEO urls
		{
			// DEADLOCK RISK!
			// However, as of right now there are no other write locks
			let mut guard_posts = self.posts.write().unwrap();
			let mut guard_post_excerpts = self.post_excerpts.write().unwrap();
			let mut guard_seo_urls = self.seo_urls.write().unwrap();
			let mut guard_seo_urls_historic = self.seo_urls_historic.write().unwrap();

			// Make sure the collections are empty
			guard_posts.clear();
			guard_post_excerpts.clear();
			guard_seo_urls.clear();
			guard_seo_urls_historic.clear();

			for mut post in blog_posts {
				// This is the main seo url for this post
				guard_seo_urls.insert(post.url_canonical.to_lowercase(), post.id);

				// Every post can have a number of historic seo urls
				for post_seo_url in post.url_historic.as_slice() {
					guard_seo_urls_historic.insert(post_seo_url.to_lowercase(), post.id);
				}

				// We will overwrite the content after we have replaced all snippets that we can find
				let mut modified_content = post.content.clone();

				// Replace any snippets inside the posts content
				for cap in regex.captures_iter(&post.content) {
					//println!("Matched key {:?}, tail: {:?}", &cap["key"], &cap["tail"]);

					// Do we have a snippet with that name?
					// Could make this into a hash map...
					for snippet in &snippets {
						if snippet.name == &cap["key"] {
							let replacement = snippet.get_replacement(&cap["tail"]);

							// Replace the occurrence in the posts content with the provided string
							modified_content = modified_content.replace(&cap[0], &replacement);
						}
					}
				}

				// Overwrite content
				post.content = modified_content;

				// Push excerpt to post_excerpt map
				guard_post_excerpts.insert(post.id, post.get_excerpt());

				// Push to posts map
				guard_posts.insert(post.id, post);
			}
		}

		Ok(post_count)
	}

	/// This function will create the sitemap for our blog
	fn reload_sitemap(&self, posts: &Vec<Post>) {
		let base_url = format!("https://{}/", config_get_string("fqdn"));
		let mut locs = Vec::new();
		let mut guard_tag_2_posts = self.tag_2_posts.write().unwrap();

		// Clear out data
		guard_tag_2_posts.clear();

		// Gather all post locations
		for post in posts {
			// Gather pictures for this post
			let mut img_locs = Vec::new();
			for image in &post.media {
				if !image.source.contains("nomadicdays.org") { continue; }
				img_locs.push({
					SiteMapImage {
						loc: image.source.clone(),
						title: {
							if image.title != "" { Some(image.title.clone()) } else { None }
						},
						caption: {
							if image.caption != "" { Some(image.caption.clone()) } else { None }
						},
					}
				});
			}

			// Create the post location including all it's images
			locs.push(SiteMapUrl {
				loc: format!("{}{}", base_url, post.url_canonical),
				lastmod: post.date_modified,
				changefreq: None,
				priority: Some(String::from("0.9")),
				images: {
					if img_locs.len() > 0 { Some(img_locs) } else { None }
				},
			});

			// For every tag this post has, store the post_id in a lookup map
			for tag in &post.tags {
				// Since this might be shared as an URL somewhere, it is better to make sure there are no spaces in those tags
				let tag_encoded = tag.replace(" ", "-");

				if let Some(vec) = guard_tag_2_posts.get_mut(&tag_encoded) {
					vec.push(post.id);
					continue;
				}
				guard_tag_2_posts.insert(tag_encoded, vec![post.id]);
			}
		}

		// Fake the tag page time for now - could find the newest timestamp of the contained posts though...
		let time = match SystemTime::now().duration_since(UNIX_EPOCH) {
			Ok(tmp) => tmp.as_secs() - 604800,
			_ => 0
		};

		// Compile all tags into the sitemap
		let per_page = config_get_i64("posts_per_page") as u32;
		for (tag, posts) in guard_tag_2_posts.iter_mut() {
			let pages = (posts.len() as f32 / per_page as f32).ceil() as u32;
			let mut page = 0u32;

			while page < pages {
				page += 1;

				locs.push(SiteMapUrl {
					loc: {
						if page == 1 { format!("{}tag/{}", base_url, tag.clone()) } else { format!("{}tag/{}?p={}", base_url, tag.clone(), page) }
					},
					lastmod: time,
					changefreq: None,
					priority: Some(String::from("0.5")),
					images: None,
				});
			}
		}

		// Compile the sitemap and cache it
		self.cache.cache_sitemap(SiteMap { content: Some(locs) });
	}

	/// Load all menus from SQL
	fn reload_menus(&self, db: &mysql::Pool) -> Result<usize, io::Error> {
		let menus = match menu::load_menus_from_sql(db) {
			Some(tmp) => { tmp }
			_ => { return Ok(0); }
		};
		let menu_count = menus.len();

		// CRITICAL SECTION: Load blog menus
		{
			let mut guard_menus = self.menus.write().unwrap();

			// Make sure the collections are empty
			guard_menus.clear();

			for menu in menus {
				guard_menus.insert(menu.name, menu.items);
			}
		}

		Ok(menu_count)
	}

	/// Load all menus from SQL
	fn reload_redirects(&self, db: &mysql::Pool) -> Result<usize, io::Error> {
		let redirects = match redirect::load_redirects_from_sql(db) {
			Some(tmp) => { tmp }
			_ => { return Ok(0); }
		};
		let redirect_count = redirects.len();

		// CRITICAL SECTION: Load blog redirects
		{
			let mut guard_redirects = self.redirects.write().unwrap();

			// Make sure the collections are empty
			guard_redirects.clear();

			for redirect in redirects {
				guard_redirects.insert(redirect.name, redirect.target);
			}
		}

		Ok(redirect_count)
	}

	/// Load all tags from SQL
	fn reload_tags(&self, db: &mysql::Pool) -> Result<usize, io::Error> {
		let tags = match tag::load_tags_from_sql(db) {
			Ok(tmp) => { tmp }
			_ => { return Ok(0); }
		};
		let tag_count = tags.len();

		// CRITICAL SECTION: Load blog tags
		{
			let mut guard_tags = self.tags.write().unwrap();

			// Make sure the collections are empty
			guard_tags.clear();

			for tag in tags {
				guard_tags.insert(tag.id.clone(), tag);
			}
		}

		Ok(tag_count)
	}

	/// Load all comments from SQL
	fn reload_comments(&self, db: &mysql::Pool) -> Result<usize, io::Error> {
		let comments = match comment::load_comments_from_sql(db) {
			Ok(tmp) => { tmp }
			_ => { return Ok(0); }
		};
		let comment_count = comments.len();

		// CRITICAL SECTION: Load blog comments
		{
			let mut guard_comments = self.comments.write().unwrap();

			// Make sure the collections are empty
			guard_comments.clear();

			for comment in comments {
				// Check if that post already has comments
				match guard_comments.get_mut(&comment.post_id) {
					Some(vec) => {
						vec.push(comment);
					}
					_ => {
						guard_comments.insert(comment.post_id, vec![comment]);
					}
				}
			}
		}

		Ok(comment_count)
	}

	// ------------------------------------------------------------------
	// ------------------------ GETTER FUNCTIONS ------------------------
	// ------------------------------------------------------------------

	/// Retrieve a menu by its key
	///
	/// This function will `lock` (read)
	fn get_menu(&self, key: &str) -> Option<Vec<menu::MenuItem>> {
		// Assume we have no menus
		let guard = match self.menus.read() {
			Ok(tmp) => { tmp }
			_ => { return None; }
		};

		match guard.get(key) {
			Some(menu) => { Some((*menu).clone()) }
			_ => { None }
		}
	}

	/// Retrieve a post by its key
	///
	/// This function will `lock` (read)
	fn get_post(&self, key: u32) -> Option<Post> {
		// Crash is intentional as we cannot operate a blog without access to posts
		let guard = self.posts.read().unwrap();

		match guard.get(&key) {
			Some(post) => { Some(post.clone()) }
			_ => { None }
		}
	}

	/// Retrieve post excerpts for a given tag
	///
	/// This function will `lock` (read)
	fn get_post_excerpts_by_tag(&self, tag_id: &str, limit: u32) -> Vec<PostExcerpt> {
		let guard_tag_2_posts = self.tag_2_posts.read().unwrap();

		match guard_tag_2_posts.get(tag_id) {
			Some(tmp) => {
				return self.get_post_excerpts(&self.get_pagination_slice(&tmp, 0, limit));
			}
			_ => {}
		}

		vec![]
	}

	/// Retrieve post excerpts by their keys
	///
	/// This function will `lock` (read)
	fn get_post_excerpts(&self, keys: &Vec<u32>) -> Vec<PostExcerpt> {
		// Create an empty vectors to hold requested excerpts
		let mut excerpts = Vec::<PostExcerpt>::with_capacity(keys.len());

		// Crash is intentional as we cannot operate a blog without access to posts
		let guard = self.post_excerpts.read().unwrap();

		for key in keys {
			match guard.get(&key) {
				Some(post_excerpt) => {
					excerpts.push(post_excerpt.clone());
				}
				_ => {}
			}
		}

		excerpts
	}

	/// Do a lookup to check if we have the blog post key for a given seo url string.
	///
	/// This function will `lock` (read, read)
	///
	/// Should we find a key for the given url we will return the matching post using `get_post()`
	fn get_post_by_seo_url(&self, seo_url: &str) -> u32 {
		let mut post_key = 0;

		// CRITICAL SECTION: Lookup the canonical seo url table
		{
			let seo_url_lower = seo_url.to_lowercase();
			let guard_seo_urls = self.seo_urls.read().unwrap();
			match guard_seo_urls.get(seo_url_lower.as_str()) {
				Some(val) => { post_key = *val; }
				_ => {}
			}
		}

		// CRITICAL SECTION: Lookup the historical seo url table
		if post_key == 0
		{
			let guard_seo_urls_historic = self.seo_urls_historic.read().unwrap();
			match guard_seo_urls_historic.get(seo_url) {
				Some(val) => { post_key = *val; }
				_ => {}
			}
		}

		post_key
	}

	/// Retrieve a `Tag` by its name
	///
	/// This function will `lock` (read)
	fn get_tag(&self, tag_id: &str) -> Option<Tag> {
		// Crash is intentional as we cannot operate a blog without access to tags
		let guard = self.tags.read().unwrap();

		match guard.get(tag_id) {
			Some(tag) => { Some(tag.clone()) }
			_ => { None }
		}
	}

	/// Returns a list of all tags currently in use
	pub fn get_all_in_use_tags(&self) -> Vec<String> {
		let guard = self.tag_2_posts.read().unwrap();

		let mut tmp = vec![];
		for (tag, _posts) in guard.iter() {
			tmp.push(tag.clone());
		}

		tmp
	}

	fn get_post_comments(&self, post_id: u32) -> Option<Vec<Comment>> {
		let guard = self.comments.read().unwrap();

		match guard.get(&post_id) {
			Some(comments) => {
				Some(comments.clone())
			}
			_ => { None }
		}
	}

	/// Do a lookup in our redirect table and find the correct target url
	pub fn lookup_redirect(&self, name: &str) -> String {
		match self.redirects.read() {
			Ok(guard) => {
				match guard.get(name) {
					Some(val) => { return val.clone(); }
					_ => {}
				}
			}
			_ => {}
		}

		format!("https://{}", config_get_string("fqdn"))
	}

	// ------------------------------------------------------------------
	// ------------------- CONTEXT CREATING FUNCTIONS -------------------
	// ------------------------------------------------------------------

	/// Create the basic data every context object will need
	#[inline(always)]
	fn create_base_context(&self) -> Context {
		Context {
			title: Some(config_get_string("title")),
			subtitle: Some(config_get_string("subtitle")),
			meta_title: Some(config_get_string("meta_title")),
			meta_description: Some(config_get_string("meta_description")),
			locale: Some(config_get_string("locale")),
			canonical: Some(format!("https://{}/", config_get_string("fqdn"))),
			time: self.get_time_in_secs(),

			// -- social --
			facebook_app_id: Some(config_get_string("facebook_app_id")),
			facebook_user: Some(config_get_string("facebook_user")),
			instagram_user: Some(config_get_string("instagram_user")),
			twitter_user: Some(config_get_string("twitter_user")),
			youtube_channel: Some(config_get_string("youtube_channel")),

			// -- menus --
			main_menu: self.get_menu("main"),

			// -- excerpts of posts with certain tags --
			excerpts_tag_1: None,
			excerpts_tag_2: None,
			excerpts_tag_3: None,
			excerpts_tag_4: None,
			excerpts_tag_5: None,

			// -- site: POST --
			post: None,
			post_related: None,
			post_comments: None,

			// -- site: INDEX --
			instagram_posts: None,
			pinterest_posts: None,
			latest_posts: None,
			featured_posts: None,

			// -- site: SEARCH & TAG (category) --
			tag: None,
			tag_id: None,
			search_string: None,
			post_list: None,
			page_current: 0,
			page_total: 0,
		}
	}


	// ------------------------------------------------------------------
	// ---------------------- RENDER HTML FUNCTIONS ---------------------
	// ------------------------------------------------------------------

	/// Create context for the index page
	pub fn get_html_base(&self, tera: &web::Data<Arc<tera::Tera>>, template: &str) -> Result<String, String> {
		// The identifier we will use to check for a cached version
		let cache_key = format!("base_{}", template);

		// Check if the HTML for this post is cached
		match self.cache.get_html(&cache_key) {
			Some(html) => return Ok(html),
			_ => {}
		}

		let mut context = self.create_base_context();

		// Instagram posts
		context.instagram_posts = self.cache.get_instagram_posts();

		// Pinterest posts
		context.pinterest_posts = self.cache.get_pinterest_posts();

		// Latest & Featured posts
		context.latest_posts = self.cache.get_latest_posts();
		context.featured_posts = self.cache.get_featured_posts();

		// Excerpts for up to 5 configurable tags
		context.excerpts_tag_1 = self.cache.get_posts_by_tag(1);
		context.excerpts_tag_2 = self.cache.get_posts_by_tag(2);
		context.excerpts_tag_3 = self.cache.get_posts_by_tag(3);
		context.excerpts_tag_4 = self.cache.get_posts_by_tag(4);
		context.excerpts_tag_5 = self.cache.get_posts_by_tag(5);

		// Render the template
		match self.render_template(tera, template, &context) {
			Ok(html) => {
				// Cache the HTML output
				self.cache.cache_html(cache_key, html.clone());

				Ok(html)
			},
			Err(err) => Err(err)
		}
	}

	/// Get the HTML for a post. The HTML may be fetched from the cache.
	pub fn get_html_post(&self, url: &str, remote_ip: String, user_agent: String, referer: String, tera: &web::Data<Arc<tera::Tera>>) -> Option<String> {

		// Lookup the SEO url
		let post_key = self.get_post_by_seo_url(url);

		// The identifier we will use to check for a cached version
		let cache_key = format!("post_{}", post_key);

		// Check if the HTML for this post is cached
		match self.cache.get_html(&cache_key) {
			Some(html) => {
				self.message_post_viewed(post_key, self.get_time_in_secs(), remote_ip, user_agent, referer);
				return Some(html)
			}
			_ => {}
		}

		// Create context for template rendering
		let mut context = self.create_base_context();

		// Did we match a blog post for the SEO url?
		if post_key > 0 {
			context.post = self.get_post(post_key);
		}

		// Set the canonical url and fetch related posts
		match &context.post {
			Some(tmp) => {
				// Log the post view by sending a post view message over the queue
				self.message_post_viewed(tmp.id, context.time, remote_ip, user_agent, referer);

				// Canonical URL
				context.canonical = Some(format!("https://{}/{}", config_get_string("fqdn"), tmp.url_canonical));

				// Copy over meta title & meta description
				context.meta_title = Some(tmp.meta_title.clone());
				context.meta_description = Some(tmp.meta_description.clone());

				// Check if we have got related posts
				if tmp.related_posts.len() > 0
				{
					context.post_related = Some(self.get_post_excerpts(&tmp.related_posts));
				}

				// Check if we have got comments for this post
				context.post_comments = self.get_post_comments(tmp.id);
			}
			_ => { return None; }
		}

		// Render the template
		match self.render_template(tera, "post.html", &context) {
			Ok(html) => {
				// Cache the HTML output
				self.cache.cache_html(cache_key, html.clone());

				Some(html)
			},
			Err(err) => Some(err)
		}
	}

	/// Get the HTML for a search. This is not yet cached.
	pub fn get_html_search(&self, db: &mysql::Pool, tera: &web::Data<Arc<tera::Tera>>, search_string: String, page: u32) -> Result<String, String> {
		let mut context = self.create_base_context();

		match crate::blog::post::fetch_posts_by_search_string(db, &search_string) {
			Ok(tmp) => {
				let per_page = config_get_i64("posts_per_page") as u32;
				context.page_current = page;
				context.page_total = (tmp.len() as f32 / per_page as f32).ceil() as u32;
				context.post_list = Some(self.get_post_excerpts(&self.get_pagination_slice(&tmp, page, per_page)));
			}
			_ => {}
		}
		context.search_string = Some(search_string.clone());
		let page_param = if page > 0 { format!("&p={}", page + 1) } else { String::from("") };
		context.canonical = Some(format!("https://{}/search?q={}{}", config_get_string("fqdn"), search_string, page_param));
		//TODO: may need URL encode for search string?? Tera template may do something to it

		// Render the template
		self.render_template(tera, "post_list.html", &context)
	}

	/// Get the HTML for a tag page. The HTML may be fetched from the cache.
	pub fn get_html_tag(&self, _db: &mysql::Pool, tera: &web::Data<Arc<tera::Tera>>, tag_id: String, page: u32) -> Result<String, String> {

		// The identifier we will use to check for a cached version
		let cache_key = format!("tag_{}_{}", tag_id, page);

		// Check if the HTML for this tag is cached
		match self.cache.get_html(&cache_key) {
			Some(html) => return Ok(html),
			_ => {}
		}

		let mut context = self.create_base_context();

		let guard_tag_2_posts = self.tag_2_posts.read().unwrap();

		match guard_tag_2_posts.get(&tag_id) {
			Some(tmp) => {
				let per_page = config_get_i64("posts_per_page") as u32;
				context.page_current = page;
				context.page_total = (tmp.len() as f32 / per_page as f32).ceil() as u32;
				context.post_list = Some(self.get_post_excerpts(&self.get_pagination_slice(&tmp, page, per_page)));
			}
			_ => {}
		}
		context.tag = self.get_tag(&tag_id);
		context.tag_id = Some(tag_id.clone());
		let page_param = if page > 0 { format!("?p={}", page + 1) } else { String::from("") };
		context.canonical = Some(format!("https://{}/tag/{}{}", config_get_string("fqdn"), tag_id, page_param));

		// If we have got some more data for this tag, use it to set custom meta title and description
		match &context.tag {
			Some(tag) => {
				if tag.meta_title.len() > 0 {
					context.meta_title = Some(tag.meta_title.clone());
				}
				if tag.meta_description.len() > 0 {
					context.meta_description = Some(tag.meta_description.clone());
				}
			}
			_ => {}
		}

		// Render the template
		match self.render_template(tera, "post_list.html", &context) {
			Ok(html) => {
				// Cache the HTML output
				self.cache.cache_html(cache_key, html.clone());

				Ok(html)
			},
			Err(err) => Err(err)
		}
	}

	/// Get the HTML for the site map. The HTML may be fetched from the cache.
	pub fn get_html_site_map(&self, tera: &web::Data<Arc<tera::Tera>>) -> Result<String, String> {

		// The identifier we will use to check for a cached version
		let cache_key = format!("site_map");

		// Check if the HTML for this tag is cached
		match self.cache.get_html(&cache_key) {
			Some(html) => return Ok(html),
			_ => {}
		}

		// Serialize context for tera
		let tera_context = match tera::Context::from_serialize(self.cache.get_site_map()).map_err(|_| error::ErrorInternalServerError("Template context error")) {
			Ok(tmp) => tmp,
			Err(err) => {
				return Err(format!("Template context error: {}", err.to_string()));
			}
		};

		// Render the template
		match tera.render("sitemap.xml", &tera_context) {
			Ok(html) => {
				// Cache the HTML output
				self.cache.cache_html(cache_key, html.clone());

				Ok(html)
			},
			Err(err) => Err(format!("Template render error: {}", err.to_string()))
		}
	}

	/// Get the HTML for the rss feed. The HTML may be fetched from the cache.
	pub fn get_html_rss_feed(&self, tera: &web::Data<Arc<tera::Tera>>) -> Result<String, String> {

		// The identifier we will use to check for a cached version
		let cache_key = format!("rss_feed");

		// Check if the HTML for this tag is cached
		match self.cache.get_html(&cache_key) {
			Some(html) => return Ok(html),
			_ => {}
		}

		// Setup context for the RSS feed
		let mut context = self.create_base_context();
		context.latest_posts = self.cache.get_latest_posts();

		// Render the template
		match self.render_template(tera, "feed.rss", &context) {
			Ok(html) => {
				// Cache the HTML output
				self.cache.cache_html(cache_key, html.clone());

				Ok(html)
			},
			Err(err) => Err(err)
		}
	}

	// ------------------------------------------------------------------
	// ----------------------- UTILITY FUNCTIONS ------------------------
	// ------------------------------------------------------------------

	/// Get the current unix time in seconds
	fn get_time_in_secs(&self) -> u64 {
		match SystemTime::now().duration_since(UNIX_EPOCH) {
			Ok(tmp) => tmp.as_secs(),
			_ => 0
		}
	}

	/// This message will create a post view
	fn message_post_viewed(&self, post_id: u32, viewed_at: u64, remote_ip: String, user_agent: String, referer: String) {
		match self.messages.lock() {
			Ok(mut guard) => {
				guard.push(BlogMessage::PostView { post_id, viewed_at, remote_ip, user_agent, referer });
			}
			_ => { println!("Message guard cannot be locked!"); }
		}
	}

	/// Try to find a slice in a vector
	#[inline(always)]
	fn get_pagination_slice(&self, source: &Vec<u32>, page: u32, per_page: u32) -> Vec<u32> {
		let mut slice = Vec::new();

		// Calculate limits
		let offset = per_page * page;
		let limit = offset + per_page;

		let mut index = 0;
		for i in source {
			if index >= offset { slice.push(*i); }
			index += 1;
			if index == limit { break; }
		}

		slice
	}

	pub fn invalidate_html_cache(&self) -> Result<usize, io::Error> {
		self.cache.reset_html_cache();
		Ok(1)
	}

	/// Render a template using the provided context
	fn render_template(&self, tera: &web::Data<Arc<tera::Tera>>, template_name: &str, context: &Context) -> Result<String, String> {
		// Serialize context for tera
		let tera_context = match tera::Context::from_serialize(context).map_err(|_| error::ErrorInternalServerError("Template context error")) {
			Ok(tmp) => tmp,
			Err(err) => {
				return Err(format!("Template context error: {}", err.to_string()));
			}
		};

		// Render the template
		match tera.render(template_name, &tera_context) {
			Ok(tmp) => Ok(tmp),
			Err(err) => Err(format!("Template render error: {}", err.to_string()))
		}
	}

	/// This function will check the cached items
	///
	/// Once a cache item's life time expires, it will be reloaded
	pub fn maintenance_task(&self, db: &mysql::Pool) {

		// Check cache Pinterest, Instagram, featured and latest posts
		self.cache.cache_pinterest_posts();
		self.cache.cache_instagram_posts();
		self.cache.cache_latest_posts(&self, db);
		self.cache.cache_featured_posts(&self, db);
		self.cache.cache_posts_by_tag(&self, 1, config_get_string("cached_tag_1").as_str());
		self.cache.cache_posts_by_tag(&self, 2, config_get_string("cached_tag_2").as_str());
		self.cache.cache_posts_by_tag(&self, 3, config_get_string("cached_tag_3").as_str());
		self.cache.cache_posts_by_tag(&self, 4, config_get_string("cached_tag_4").as_str());
		self.cache.cache_posts_by_tag(&self, 5, config_get_string("cached_tag_5").as_str());

		// Process messages handled by the queue
		{
			let mut views = Vec::<(u32, u64, String, String, String)>::new();

			match self.messages.lock() {
				Ok(mut guard) => {
					for msg in guard.iter() {
						match msg {
							BlogMessage::PostView { post_id, viewed_at, remote_ip, user_agent, referer } => {
								views.push((*post_id, *viewed_at, remote_ip.clone(), user_agent.clone(), referer.clone()));
							}
						}
					}
					// There is nothing but view messages atm so we can clear it
					guard.clear();
				}
				_ => {}
			}

			if views.len() > 0 {
				crate::blog::post::log_post_views(db, &views)
			}
		}
	}
}