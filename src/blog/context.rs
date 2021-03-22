use crate::blog::types::menu;
use crate::blog::types::post::{PostExcerpt, Post};
use crate::blog::types::comment::Comment;
use crate::app::utils::{InstagramPostCompact, PinterestPostCompact};
use crate::blog::types::tag::Tag;

/// Context is required by the Tera template engine
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Context {
	pub title: Option<String>,
	pub subtitle: Option<String>,
	pub meta_title: Option<String>,
	pub meta_description: Option<String>,
	pub locale: Option<String>,
	pub canonical: Option<String>,
	pub time: u64,

	// -- social --
	pub facebook_app_id: Option<String>,
	pub facebook_user: Option<String>,
	pub instagram_user: Option<String>,
	pub twitter_user: Option<String>,
	pub youtube_channel: Option<String>,

	// -- menus --
	pub main_menu: Option<Vec<menu::MenuItem>>,

	// -- excerpts of posts with certain tags --
	pub excerpts_tag_1: Option<Vec<PostExcerpt>>,
	pub excerpts_tag_2: Option<Vec<PostExcerpt>>,
	pub excerpts_tag_3: Option<Vec<PostExcerpt>>,
	pub excerpts_tag_4: Option<Vec<PostExcerpt>>,
	pub excerpts_tag_5: Option<Vec<PostExcerpt>>,

	// -- site: POST --
	pub post: Option<Post>,
	pub post_related: Option<Vec<PostExcerpt>>,
	pub post_comments: Option<Vec<Comment>>,

	// -- site: INDEX --
	pub instagram_posts: Option<Vec<InstagramPostCompact>>,
	pub pinterest_posts: Option<Vec<PinterestPostCompact>>,
	pub latest_posts: Option<Vec<PostExcerpt>>,
	pub featured_posts: Option<Vec<PostExcerpt>>,

	// -- site: SEARCH & TAG (category) --
	pub tag: Option<Tag>,
	pub tag_id: Option<String>,
	pub search_string: Option<String>,
	pub post_list: Option<Vec<PostExcerpt>>,
	pub page_current: u32,
	pub page_total: u32,
}


// Index page

// Post page

// Search page

// Tag page

// Sitemap

// RSS feed

//TODO: clean up context setup - make different contexts for different things?
// Or just move the base content here? Could implement default to make it a little cleaner