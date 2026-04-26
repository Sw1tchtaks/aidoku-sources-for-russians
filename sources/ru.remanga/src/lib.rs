#![no_std]
extern crate alloc;

mod api;
mod models;

use aidoku::helpers::uri::encode_uri_component;
use aidoku::imports::net::{Request, TimeUnit, set_rate_limit};
use aidoku::prelude::*;
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, HashMap, ImageRequestProvider, Listing,
	ListingProvider, Manga, MangaPageResult, Page, PageContent, PageContext, Result, Source,
	WebLoginHandler,
	alloc::{String, Vec},
};
use alloc::format;
use alloc::string::ToString;

use api::{SITE_URL, apply_headers, get_json, store_token};
use models::{CatalogItem, ChapterDto, ChapterPagesContent, DetailsItem, Envelope, Page as ApiPage, flatten_pages};

const PAGE_SIZE: i32 = 30;

struct Remanga;

impl Source for Remanga {
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
		let path = if let Some(q) = query.as_ref().filter(|q| !q.trim().is_empty()) {
			format!(
				"/api/v2/search/?query={}&page={page}&count={PAGE_SIZE}",
				encode_uri_component(q.as_str())
			)
		} else {
			format!("/api/v2/search/catalog/?page={page}&count={PAGE_SIZE}&ordering=-rating")
		};
		let envelope: Envelope<ApiPage<CatalogItem>> = get_json(&path)?;
		let payload = envelope.content;
		let entries: Vec<Manga> = payload.results.into_iter().map(|c| c.into_manga()).collect();
		Ok(MangaPageResult {
			has_next_page: payload.next.is_some(),
			entries,
		})
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let dir = manga.key.clone();
		let mut updated = manga;

		let envelope: Envelope<DetailsItem> = get_json(&format!("/api/v2/titles/{dir}/"))?;
		let details = envelope.content;
		let branch_id = details.primary_branch_id();

		if needs_details {
			updated = details.merge_into(updated);
		} else {
			// Always carry the canonical key forward.
			updated.key = details.dir.clone();
		}

		if needs_chapters {
			let mut chapters: Vec<Chapter> = Vec::new();
			if let Some(branch_id) = branch_id {
				// Walk all branch pages until next == null. Remanga returns oldest-first
				// when ordering=index ascending; we ask -index for newest first.
				let mut page = 1;
				loop {
					let path = format!(
						"/api/v2/titles/chapters/?branch_id={branch_id}&ordering=-index&page={page}&count=200"
					);
					let envelope: Envelope<ApiPage<ChapterDto>> = get_json(&path)?;
					let payload = envelope.content;
					let exhausted = payload.next.is_none() || payload.results.is_empty();
					for c in payload.results {
						chapters.push(c.into_chapter(&dir));
					}
					if exhausted {
						break;
					}
					page += 1;
					if page > 50 {
						// Safety stop — 200 * 50 = 10k chapters cap.
						break;
					}
				}
			}
			updated.chapters = Some(chapters);
		}

		Ok(updated)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let envelope: Envelope<ChapterPagesContent> =
			get_json(&format!("/api/v2/titles/chapters/{}/", chapter.key))?;
		let urls = flatten_pages(&envelope.content.pages);
		Ok(urls
			.into_iter()
			.map(|u| Page {
				content: PageContent::url(u),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for Remanga {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let ordering = match listing.id.as_str() {
			"latest" => "-chapter_date",
			"new" => "-id",
			_ => "-rating",
		};
		let path =
			format!("/api/v2/search/catalog/?page={page}&count={PAGE_SIZE}&ordering={ordering}");
		let envelope: Envelope<ApiPage<CatalogItem>> = get_json(&path)?;
		let payload = envelope.content;
		let entries: Vec<Manga> = payload.results.into_iter().map(|c| c.into_manga()).collect();
		Ok(MangaPageResult {
			has_next_page: payload.next.is_some(),
			entries,
		})
	}
}

impl ImageRequestProvider for Remanga {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(apply_headers(Request::get(url)?))
	}
}

impl DeepLinkHandler for Remanga {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		// Format: https://remanga.org/manga/{slug}[/ch{id}]
		let Some(idx) = url.find("/manga/") else {
			return Ok(None);
		};
		let rest = &url[idx + "/manga/".len()..];
		let mut parts = rest.split('/');
		let Some(slug) = parts.next() else {
			return Ok(None);
		};
		if slug.is_empty() {
			return Ok(None);
		}
		// Optional /ch<id> chapter segment.
		if let Some(ch) = parts.next() {
			if let Some(id) = ch.strip_prefix("ch") {
				return Ok(Some(DeepLinkResult::Chapter {
					manga_key: slug.to_string(),
					key: id.to_string(),
				}));
			}
		}
		Ok(Some(DeepLinkResult::Manga {
			key: slug.to_string(),
		}))
	}
}

impl WebLoginHandler for Remanga {
	fn handle_web_login(&self, _key: String, cookies: HashMap<String, String>) -> Result<bool> {
		// Remanga's web login sets the JWT in the `user` cookie value (URL-encoded JSON
		// containing { "access_token": "..." }). Try a few likely cookie names; if any
		// look like a JWT, store it for later use as the bearer token.
		for candidate in ["access_token", "token", "authToken", "jwt"] {
			if let Some(v) = cookies.get(candidate) {
				if !v.is_empty() {
					store_token(v);
					println!("[remanga] login: stored token from cookie {candidate}");
					return Ok(true);
				}
			}
		}
		// Fall back to the `user` cookie which usually holds an URL-encoded JSON blob.
		if let Some(user) = cookies.get("user") {
			if let Some(token) = extract_token_from_user_cookie(user) {
				store_token(&token);
				println!("[remanga] login: stored token from user cookie");
				return Ok(true);
			}
		}
		println!(
			"[remanga] login: no recognised auth cookie among {:?}",
			cookies.keys().collect::<Vec<_>>()
		);
		Ok(false)
	}
}

fn extract_token_from_user_cookie(value: &str) -> Option<String> {
	// Attempt to URL-decode then JSON-decode; pull access_token / token field.
	let decoded = aidoku::helpers::uri::decode_uri(value);
	let v: serde_json::Value = serde_json::from_str(&decoded).ok()?;
	for key in ["access_token", "token", "authToken"] {
		if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
			return Some(s.to_string());
		}
	}
	None
}

register_source!(
	Remanga,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler,
	WebLoginHandler
);
