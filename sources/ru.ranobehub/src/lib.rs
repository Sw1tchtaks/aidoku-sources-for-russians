#![no_std]
extern crate alloc;

mod models;

use aidoku::helpers::uri::encode_uri_component;
use aidoku::imports::net::{Request, TimeUnit, set_rate_limit};
use aidoku::prelude::*;
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent, HomeComponentValue,
	HomeLayout, ImageRequestProvider, Link, Listing, ListingKind, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, PageContext, Result, Source,
	alloc::{String, Vec},
};
use alloc::format;
use alloc::string::ToString;
use serde::de::DeserializeOwned;

use models::{
	ChapterContentEnvelope, ContentsEnvelope, DetailsEnvelope, FulltextSection, SearchEnvelope,
	strip_html,
};

const SITE_URL: &str = "https://ranobehub.org";
const API_URL: &str = "https://ranobehub.org/api";

fn fetch_json<T: DeserializeOwned>(url: &str) -> Result<T> {
	let response = Request::get(url)?
		.header(
			"User-Agent",
			"Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
		)
		.header("Referer", SITE_URL)
		.header("Accept", "application/json")
		.send()?;
	let status = response.status_code();
	let bytes = response.get_data()?;
	if !(200..300).contains(&status) {
		let preview = preview(&bytes);
		println!("[ranobehub] HTTP {status} for {url}: {preview}");
		return Err(error!("RanobeHub HTTP {status}"));
	}
	serde_json::from_slice(&bytes).map_err(|e| {
		let preview = preview(&bytes);
		error!("RanobeHub decode {url}: {e}; body: {preview}")
	})
}

fn preview(bytes: &[u8]) -> String {
	let n = bytes.len().min(300);
	String::from_utf8_lossy(&bytes[..n]).into_owned()
}

struct RanobeHub;

impl Source for RanobeHub {
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
		let trimmed = query
			.as_ref()
			.map(|q| q.trim().to_string())
			.filter(|q| !q.is_empty());
		if let Some(q) = trimmed {
			// Fulltext search returns ungrouped sections; pick the one keyed
			// "ranobes" / "ranobe". Search has no native pagination on the API,
			// so page > 1 returns empty.
			if page != 1 {
				return Ok(MangaPageResult::default());
			}
			let url = format!(
				"{API_URL}/fulltext/global?query={}",
				encode_uri_component(q.as_str())
			);
			let sections: Vec<FulltextSection> = fetch_json(&url)?;
			let mut entries: Vec<Manga> = Vec::new();
			for s in sections {
				let key = s
					.meta
					.as_ref()
					.and_then(|m| m.key.clone())
					.unwrap_or_default();
				if !(key == "ranobes" || key == "ranobe" || key == "books") {
					continue;
				}
				for item in s.data {
					entries.push(item.into_manga());
				}
			}
			Ok(MangaPageResult {
				entries,
				has_next_page: false,
			})
		} else {
			let url = format!("{API_URL}/search?page={page}");
			let envelope: SearchEnvelope = fetch_json(&url)?;
			let entries: Vec<Manga> = envelope.resource.into_iter().map(|i| i.into_manga()).collect();
			let has_next_page = envelope
				.pagination
				.map(|p| p.current_page < p.last_page)
				.unwrap_or(false);
			Ok(MangaPageResult {
				entries,
				has_next_page,
			})
		}
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let id = manga.key.clone();
		let mut updated = manga;

		if needs_details {
			let url = format!("{API_URL}/ranobe/{id}");
			let envelope: DetailsEnvelope = fetch_json(&url)?;
			updated = envelope.data.merge_into(updated);
		}

		if needs_chapters {
			let url = format!("{API_URL}/ranobe/{id}/contents");
			let envelope: ContentsEnvelope = fetch_json(&url)?;
			let mut chapters: Vec<Chapter> = Vec::new();
			for vol in envelope.volumes {
				let vol_num = vol.num;
				for c in vol.chapters {
					chapters.push(c.into_chapter(vol_num));
				}
			}
			// Aidoku expects newest-first ordering.
			chapters.reverse();
			updated.chapters = Some(chapters);
		}

		Ok(updated)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{API_URL}/chapter/{}", chapter.key);
		let envelope: ChapterContentEnvelope = fetch_json(&url)?;
		let raw = envelope
			.chapter
			.and_then(|c| c.text)
			.unwrap_or_default();
		let text = strip_html(&raw);
		if text.is_empty() {
			println!("[ranobehub] empty chapter text for {}", chapter.key);
		}
		Ok(alloc::vec![Page {
			content: PageContent::text(text),
			..Default::default()
		}])
	}
}

impl ListingProvider for RanobeHub {
	fn get_manga_list(&self, _listing: Listing, page: i32) -> Result<MangaPageResult> {
		let url = format!("{API_URL}/search?page={page}");
		let envelope: SearchEnvelope = fetch_json(&url)?;
		let entries: Vec<Manga> = envelope.resource.into_iter().map(|i| i.into_manga()).collect();
		let has_next_page = envelope
			.pagination
			.map(|p| p.current_page < p.last_page)
			.unwrap_or(false);
		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}
}

impl Home for RanobeHub {
	fn get_home(&self) -> Result<HomeLayout> {
		let url = format!("{API_URL}/search?page=1");
		let envelope: SearchEnvelope = fetch_json(&url)?;
		let entries: Vec<Manga> = envelope.resource.into_iter().map(|i| i.into_manga()).collect();
		let big_entries: Vec<Manga> = entries.iter().take(5).cloned().collect();
		let scroller_entries: Vec<Link> = entries.into_iter().skip(5).map(Link::from).collect();
		let components = alloc::vec![
			HomeComponent {
				title: Some("Популярное".to_string()),
				subtitle: None,
				value: HomeComponentValue::BigScroller {
					entries: big_entries,
					auto_scroll_interval: Some(8.0),
				},
			},
			HomeComponent {
				title: Some("Каталог".to_string()),
				subtitle: None,
				value: HomeComponentValue::Scroller {
					entries: scroller_entries,
					listing: Some(Listing {
						id: "popular".to_string(),
						name: "Каталог".to_string(),
						kind: ListingKind::Default,
					}),
				},
			},
		];
		Ok(HomeLayout { components })
	}
}

impl ImageRequestProvider for RanobeHub {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?
			.header(
				"User-Agent",
				"Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
			)
			.header("Referer", SITE_URL))
	}
}

impl DeepLinkHandler for RanobeHub {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		// /ranobe/{id}-...  or  /ranobe/{id}/{vol}/{chap}
		let Some(after) = url.split("/ranobe/").nth(1) else {
			return Ok(None);
		};
		let mut parts = after.split('/');
		let first = parts.next().unwrap_or("");
		// Numeric prefix may be followed by "-slug-text"; cut it.
		let id_str = first.split(|c: char| c == '-' || c == '?').next().unwrap_or("");
		if id_str.is_empty() || !id_str.chars().all(|c| c.is_ascii_digit()) {
			return Ok(None);
		}
		Ok(Some(DeepLinkResult::Manga {
			key: id_str.to_string(),
		}))
	}
}

register_source!(
	RanobeHub,
	ListingProvider,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
