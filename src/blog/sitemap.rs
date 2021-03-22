#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SiteMap {
	pub content: Option<Vec<SiteMapUrl>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SiteMapUrl {
	pub loc: String,
	pub lastmod: u64,
	pub changefreq: Option<String>,
	pub priority: Option<String>,
	pub images: Option<Vec<SiteMapImage>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SiteMapImage {
	pub loc: String,
	pub title: Option<String>,
	pub caption: Option<String>,
}