#![no_std]
extern crate alloc;

mod parser;

use aidoku::helpers::uri::encode_uri_component;
use aidoku::imports::defaults::defaults_get;
use aidoku::imports::html::{Document, Html};
use aidoku::imports::net::{Request, TimeUnit, set_rate_limit};
use aidoku::prelude::*;
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Listing,
	ListingProvider, Manga, MangaPageResult, Page, PageContent, PageContext, Result, Source,
	alloc::{String, Vec},
};
use alloc::format;
use alloc::string::ToString;

const DEFAULT_BASE_URL: &str = "https://ranobes.com";
const PAGE_SIZE: i32 = 10;
const CHAPTER_LIST_PAGES_CAP: i32 = 50;

fn base_url() -> String {
	let mut url = defaults_get::<String>("baseUrl").unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
	if url.ends_with('/') {
		url.pop();
	}
	url
}

fn fetch_html(url: &str) -> Result<Document> {
	let base = base_url();
	let response = Request::get(url)?
		.header(
			"User-Agent",
			"Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
		)
		.header("Referer", &base)
		.header(
			"Accept",
			"text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
		)
		.header("Accept-Language", "ru,en;q=0.9")
		.send()?;
	let status = response.status_code();
	let bytes = response.get_data()?;
	if !(200..400).contains(&status) {
		return Err(error!("Ranobes HTTP {} for {}", status, url));
	}
	Html::parse_with_url(bytes, &base).map_err(|e| error!("Ranobes parse error: {:?}", e))
}

fn parse_listing(doc: &Document) -> MangaPageResult {
	let entries = doc
		.select("div.block.story.shortstory")
		.map(|list| {
			list.into_iter()
				.filter_map(|el| parser::parse_tile(&el))
				.collect::<Vec<_>>()
		})
		.unwrap_or_default();
	let has_next_page = doc
		.select_first(".page_next a, a.page_next")
		.is_some()
		|| doc.select_first(".pages a:contains(»)").is_some();
	MangaPageResult {
		entries,
		has_next_page,
	}
}

struct Ranobes;

impl Source for Ranobes {
	fn new() -> Self {
		set_rate_limit(2, 1, TimeUnit::Seconds);
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let base = base_url();
		let url = if let Some(q) = query.as_ref().filter(|q| !q.trim().is_empty()) {
			let from = (PAGE_SIZE * (page - 1).max(0)).max(0);
			format!(
				"{base}/index.php?do=search&subaction=search&search_start={from}&full_search=0&result_from={from}&story={}",
				encode_uri_component(q.as_str())
			)
		} else {
			format!("{base}/ranobe/page/{page}/")
		};
		let doc = fetch_html(&url)?;
		Ok(parse_listing(&doc))
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let key = manga.key.clone();
		let base = base_url();
		let mut updated = manga;

		if needs_details {
			let details_url = parser::manga_url(&base, &key);
			let doc = fetch_html(&details_url)?;
			updated.url = Some(details_url);
			parser::fill_details(&doc, &mut updated);
		}

		if needs_chapters {
			let details_url = parser::manga_url(&base, &key);
			let details_doc = fetch_html(&details_url)?;
			let first_chapter_href = details_doc
				.select_first(".chapters-scroll-list a")
				.and_then(|el| el.attr("abs:href").or_else(|| el.attr("href")))
				.or_else(|| {
					details_doc
						.select_first("a[href*='/chapters/']")
						.and_then(|el| el.attr("abs:href").or_else(|| el.attr("href")))
				});
			let chapter_slug = first_chapter_href
				.as_deref()
				.and_then(parser::chapter_slug_from_link)
				.map(|s| s.to_string());
			let mut chapters: Vec<Chapter> = Vec::new();
			if let Some(slug) = chapter_slug {
				for page_num in 1..=CHAPTER_LIST_PAGES_CAP {
					let url = format!("{base}/chapters/{slug}/page/{page_num}/");
					let doc = fetch_html(&url)?;
					let rows = doc
						.select("div.cat_block.cat_line")
						.map(|list| {
							list.into_iter()
								.filter_map(|el| parser::parse_chapter_row(&el))
								.collect::<Vec<_>>()
						})
						.unwrap_or_default();
					if rows.is_empty() {
						break;
					}
					let exhausted = doc.select_first(".page_next a, a.page_next").is_none();
					chapters.extend(rows);
					if exhausted {
						break;
					}
				}
			}
			updated.chapters = Some(chapters);
		}

		Ok(updated)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let base = base_url();
		let url = parser::chapter_url(&base, &chapter.key);
		let doc = fetch_html(&url)?;
		let text = parser::extract_chapter_text(&doc);
		if text.is_empty() {
			println!("[ranobes] empty text for {url}");
		}
		Ok(alloc::vec![Page {
			content: PageContent::text(text),
			..Default::default()
		}])
	}
}

impl ListingProvider for Ranobes {
	fn get_manga_list(&self, _listing: Listing, page: i32) -> Result<MangaPageResult> {
		let base = base_url();
		let url = format!("{base}/ranobe/page/{page}/");
		let doc = fetch_html(&url)?;
		Ok(parse_listing(&doc))
	}
}

impl ImageRequestProvider for Ranobes {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		let base = base_url();
		Ok(Request::get(url)?
			.header(
				"User-Agent",
				"Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
			)
			.header("Referer", &base))
	}
}

impl DeepLinkHandler for Ranobes {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if let Some(key) = parser::url_to_manga_key(&url) {
			return Ok(Some(DeepLinkResult::Manga { key }));
		}
		if let Some(key) = parser::url_to_chapter_key(&url) {
			// We don't know the manga slug from a chapter URL alone — leave manga_key
			// blank; Aidoku will fall back to looking it up.
			return Ok(Some(DeepLinkResult::Chapter {
				manga_key: String::new(),
				key,
			}));
		}
		Ok(None)
	}
}

register_source!(
	Ranobes,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler
);
