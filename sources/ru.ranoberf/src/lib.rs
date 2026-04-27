#![no_std]
extern crate alloc;

mod models;

use aidoku::imports::html::{Document, Html};
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
use serde::Deserialize;
use serde::de::DeserializeOwned;

use models::{
	BookFull, CatalogEnvelope, ChapterPageProps, NextData, SITE_URL, merge_book_into, strip_html,
};

const PAGE_SIZE: usize = 30;

fn fetch_text(url: &str) -> Result<Vec<u8>> {
	let response = Request::get(url)?
		.header(
			"User-Agent",
			"Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
		)
		.header("Referer", SITE_URL)
		.header("Accept-Language", "ru,en;q=0.9")
		.send()?;
	let status = response.status_code();
	let bytes = response.get_data()?;
	if !(200..300).contains(&status) {
		let preview = preview(&bytes);
		println!("[ranoberf] HTTP {status} for {url}: {preview}");
		return Err(error!("Ранобэ.рф HTTP {status}"));
	}
	Ok(bytes)
}

fn fetch_json<T: DeserializeOwned>(url: &str) -> Result<T> {
	let bytes = fetch_text(url)?;
	serde_json::from_slice(&bytes).map_err(|e| {
		let preview = preview(&bytes);
		error!("Ранобэ.рф decode {url}: {e}; body: {preview}")
	})
}

fn fetch_html(url: &str) -> Result<Document> {
	let bytes = fetch_text(url)?;
	Html::parse_with_url(bytes, SITE_URL).map_err(|e| error!("Ранобэ.рф parse: {:?}", e))
}

/// Extract the JSON payload from a Next.js `<script id="__NEXT_DATA__">` tag
/// and deserialize it into the requested page-props shape.
fn extract_next_props<T: DeserializeOwned>(doc: &Document) -> Result<T> {
	let script = doc
		.select_first("script#__NEXT_DATA__")
		.ok_or(error!("Ранобэ.рф: __NEXT_DATA__ script not found"))?;
	let json = script
		.data()
		.or_else(|| script.html())
		.ok_or(error!("Ранобэ.рф: __NEXT_DATA__ script has no contents"))?;
	let next: NextData<T> = serde_json::from_str(&json)
		.map_err(|e| error!("Ранобэ.рф: __NEXT_DATA__ decode: {e}"))?;
	Ok(next.props.page_props)
}

fn preview(bytes: &[u8]) -> String {
	let n = bytes.len().min(300);
	String::from_utf8_lossy(&bytes[..n]).into_owned()
}

#[derive(Deserialize)]
struct BookPagePropsRaw {
	book: Option<BookFull>,
}

struct Ranoberf;

impl Source for Ranoberf {
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
		let url = format!("{SITE_URL}/v3/book");
		let envelope: CatalogEnvelope = fetch_json(&url)?;
		let mut items = envelope.items;

		if let Some(q) = query.as_ref().map(|q| q.trim()).filter(|q| !q.is_empty()) {
			let q_lower = q.to_lowercase();
			items.retain(|it| it.title.to_lowercase().contains(&q_lower));
		}

		let total = items.len();
		let page = page.max(1) as usize;
		let start = (page - 1) * PAGE_SIZE;
		if start >= total {
			return Ok(MangaPageResult::default());
		}
		let end = (start + PAGE_SIZE).min(total);
		let slice = items.into_iter().skip(start).take(end - start);
		let entries: Vec<Manga> = slice.map(|i| i.into_manga()).collect();
		let has_next_page = end < total;
		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let slug = manga.key.clone();
		let mut updated = manga;

		if needs_details || needs_chapters {
			let url = format!("{SITE_URL}/{slug}");
			let doc = fetch_html(&url)?;
			let props: BookPagePropsRaw = extract_next_props(&doc)?;
			let Some(mut book) = props.book else {
				return Err(error!("Ранобэ.рф: book missing in page props"));
			};

			// Take chapters out of `book` so we can move `book` into merge_book_into
			// without cloning the (potentially large) chapter list.
			let chapters_dto = book.chapters.take();
			if needs_details {
				updated = merge_book_into(book, updated);
			}
			if needs_chapters {
				let mut chapters: Vec<Chapter> = chapters_dto
					.unwrap_or_default()
					.into_iter()
					.map(|c| c.into_chapter())
					.collect();
				// API order is newest-first already (matches the site UI), so leave as-is.
				// But if some lists come oldest-first, this is the place to chapters.reverse().
				if let Some(first) = chapters.first().and_then(|c| c.chapter_number) {
					if let Some(last) = chapters.last().and_then(|c| c.chapter_number) {
						if first < last {
							chapters.reverse();
						}
					}
				}
				updated.chapters = Some(chapters);
			}
		}

		Ok(updated)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let chapter_url = chapter.url.clone().unwrap_or_else(|| {
			format!("{SITE_URL}/?chapter={}", chapter.key)
		});
		let doc = fetch_html(&chapter_url)?;
		let props: ChapterPageProps = extract_next_props(&doc)?;
		let raw = props
			.chapter
			.and_then(|c| c.extract_text())
			.unwrap_or_default();
		let text = strip_html(&raw);
		if text.is_empty() {
			println!("[ranoberf] empty chapter text for {chapter_url}");
		}
		Ok(alloc::vec![Page {
			content: PageContent::text(text),
			..Default::default()
		}])
	}
}

impl ListingProvider for Ranoberf {
	fn get_manga_list(&self, _listing: Listing, page: i32) -> Result<MangaPageResult> {
		self.get_search_manga_list(None, page, Vec::new())
	}
}

impl Home for Ranoberf {
	fn get_home(&self) -> Result<HomeLayout> {
		let first_page = self.get_search_manga_list(None, 1, Vec::new())?;
		let entries = first_page.entries;
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

impl ImageRequestProvider for Ranoberf {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?
			.header(
				"User-Agent",
				"Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
			)
			.header("Referer", SITE_URL))
	}
}

impl DeepLinkHandler for Ranoberf {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		// Site URL like https://xn--80ac9aeh6f.xn--p1ai/{slug} or
		// https://ранобэ.рф/{slug}; treat the first non-empty path segment as
		// the manga key (slug).
		let after = url
			.split("//")
			.nth(1)
			.and_then(|s| s.split('/').nth(1))
			.unwrap_or("");
		if after.is_empty() {
			return Ok(None);
		}
		Ok(Some(DeepLinkResult::Manga {
			key: after.to_string(),
		}))
	}
}

register_source!(
	Ranoberf,
	ListingProvider,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
