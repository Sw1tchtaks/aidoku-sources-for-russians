#![no_std]
extern crate alloc;

use aidoku::helpers::uri::encode_uri_component;
use aidoku::imports::html::{Document, Html};
use aidoku::imports::net::{Request, TimeUnit, set_rate_limit};
use aidoku::prelude::*;
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent,
	HomeComponentValue, HomeLayout, ImageRequestProvider, Link, Listing, ListingKind,
	ListingProvider, Manga, MangaPageResult, MangaStatus, Page, PageContent, PageContext, Result,
	Source, Viewer,
	alloc::{String, Vec},
};
use alloc::format;
use alloc::string::ToString;

const SITE_URL: &str = "https://acomics.ru";
const PAGE_SIZE: i32 = 10;
// `/list` returns 5 issues per response. Bump skip by this between fetches.
const ISSUES_PER_LIST_PAGE: i32 = 5;
// Hard cap on how many pages worth of issues we'll walk per chapter refresh.
// 200 list-pages × 5 = 1000 individual comic pages — covers the longest webcomics
// without blowing up refresh time.
const ISSUE_LIST_PAGES_CAP: i32 = 200;

fn fetch_html(url: &str) -> Result<Document> {
	let response = Request::get(url)?
		.header(
			"User-Agent",
			"Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
		)
		.header("Referer", SITE_URL)
		.header(
			"Accept",
			"text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
		)
		.header("Accept-Language", "ru,en;q=0.9")
		.send()?;
	let status = response.status_code();
	let bytes = response.get_data()?;
	if !(200..400).contains(&status) {
		return Err(error!("Acomics HTTP {status} for {url}"));
	}
	Html::parse_with_url(bytes, SITE_URL).map_err(|e| error!("Acomics parse: {:?}", e))
}

/// `/comics?skip=N` lists 10 comics per page; tile selector is
/// `section.serial-card`. Each tile has:
///   <a href="/~slug" class="cover"><img data-real-src="..."></a>
///   <h2 class="title"><a href="/~slug">…</a></h2>
fn parse_catalog(doc: &Document, base_url: &str) -> MangaPageResult {
	let entries = doc
		.select("section.serial-card")
		.map(|list| {
			list.into_iter()
				.filter_map(|el| {
					let title_link = el.select_first("h2.title a")?;
					let title = title_link
						.text()
						.filter(|s| !s.is_empty())
						.or_else(|| title_link.attr("title"))?;
					let href = title_link
						.attr("abs:href")
						.or_else(|| title_link.attr("href"))?;
					let key = url_to_key(&href)?;

					let cover = el
						.select_first("a.cover img")
						.and_then(|img| {
							img.attr("data-real-src")
								.or_else(|| img.attr("abs:data-real-src"))
								.or_else(|| img.attr("abs:src"))
								.or_else(|| img.attr("src"))
						})
						.map(|u| absolutize(&u, base_url));

					let description = el
						.select_first("p.about")
						.and_then(|e| e.text())
						.filter(|s| !s.is_empty());

					Some(Manga {
						key,
						title,
						cover,
						description,
						url: Some(format!("{base_url}/~{}", url_to_key(&href).unwrap_or_default())),
						..Default::default()
					})
				})
				.collect::<Vec<_>>()
		})
		.unwrap_or_default();

	// Has-next: next "skip" link in pagination block.
	let has_next_page = doc.select_first("a.button-next").is_some()
		|| doc
			.select_first("a[href*='skip=']")
			.and_then(|e| e.attr("href"))
			.map(|h| h.contains("skip="))
			.unwrap_or(false);

	MangaPageResult {
		entries,
		has_next_page,
	}
}

/// Strip the `~` prefix and return the comic slug.
fn url_to_key(url: &str) -> Option<String> {
	let after = url.split("/~").nth(1)?;
	let key = after.split('/').next()?;
	if key.is_empty() {
		return None;
	}
	Some(key.to_string())
}

fn absolutize(url: &str, base: &str) -> String {
	if url.starts_with("http://") || url.starts_with("https://") {
		url.to_string()
	} else if url.starts_with('/') {
		format!("{base}{url}")
	} else {
		format!("{base}/{url}")
	}
}

fn fill_details(doc: &Document, manga: &mut Manga, base_url: &str) {
	if let Some(t) = doc
		.select_first("h1.serial-title")
		.or_else(|| doc.select_first("h1"))
		.and_then(|e| e.text())
		.filter(|s| !s.is_empty())
	{
		manga.title = t;
	}
	let cover = doc
		.select_first("meta[property=\"og:image\"]")
		.and_then(|e| e.attr("content"))
		.or_else(|| {
			doc.select_first("img.serial-thumb, .serial-image img, .cover img")
				.and_then(|img| img.attr("data-real-src").or_else(|| img.attr("src")))
		})
		.map(|u| absolutize(&u, base_url));
	if cover.is_some() {
		manga.cover = cover;
	}
	manga.description = doc
		.select_first("meta[property=\"og:description\"]")
		.or_else(|| doc.select_first("meta[name=\"description\"]"))
		.and_then(|e| e.attr("content"))
		.filter(|s| !s.is_empty());

	let mut tags: Vec<String> = Vec::new();
	if let Some(cats) = doc.select(".serial-categories a, .categories a") {
		for c in cats {
			if let Some(t) = c.text() {
				let trimmed = t.trim();
				if !trimmed.is_empty() {
					tags.push(trimmed.to_string());
				}
			}
		}
	}
	if !tags.is_empty() {
		manga.tags = Some(tags);
	}

	manga.viewer = Viewer::Webtoon;
	// Acomics has a few NSFW tagged comics but most are SFW. Without a stable
	// indicator on the catalog tile, leave content rating as Safe by default.
	manga.content_rating = ContentRating::Safe;
	// No reliable status text on the public page; leave Unknown.
	manga.status = MangaStatus::Unknown;
}

/// Pull every issue image URL by walking `/~slug/list?skip=0,5,10,...`.
fn collect_issue_urls(slug: &str) -> Result<Vec<String>> {
	let mut out: Vec<String> = Vec::new();
	for page_idx in 0..ISSUE_LIST_PAGES_CAP {
		let skip = page_idx * ISSUES_PER_LIST_PAGE;
		let url = format!("{SITE_URL}/~{slug}/list?skip={skip}");
		let doc = fetch_html(&url)?;
		let urls = doc
			.select(".reader-issue img, section.reader-issue img")
			.map(|list| {
				list.into_iter()
					.filter_map(|img| img.attr("abs:src").or_else(|| img.attr("src")))
					.filter(|s| s.contains("/upload/!c/"))
					.collect::<Vec<_>>()
			})
			.unwrap_or_default();
		if urls.is_empty() {
			break;
		}
		// Skip the very first hit on every page that's actually the "current/last
		// issue" preview, which would dupe page 1 across all calls. Rely on the
		// /upload/!c/ filter — it leaves only real comic-page images.
		let new_count = urls.len();
		out.extend(urls);
		if new_count < ISSUES_PER_LIST_PAGE as usize {
			break;
		}
	}
	// Dedupe while preserving order — covers, navigation reuses sometimes.
	let mut seen: Vec<String> = Vec::new();
	for u in out {
		if !seen.iter().any(|x| x == &u) {
			seen.push(u);
		}
	}
	Ok(seen)
}

struct Acomics;

impl Source for Acomics {
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
		let skip = PAGE_SIZE * (page - 1).max(0);
		let url = if let Some(q) = query.as_ref().filter(|q| !q.trim().is_empty()) {
			format!(
				"{SITE_URL}/search?keyword={}&skip={skip}",
				encode_uri_component(q.as_str())
			)
		} else {
			format!("{SITE_URL}/comics?skip={skip}")
		};
		let doc = fetch_html(&url)?;
		Ok(parse_catalog(&doc, SITE_URL))
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let slug = manga.key.clone();
		let mut updated = manga;

		if needs_details {
			let url = format!("{SITE_URL}/~{slug}");
			let doc = fetch_html(&url)?;
			updated.url = Some(url);
			fill_details(&doc, &mut updated, SITE_URL);
		}

		if needs_chapters {
			// Acomics is a webcomic site — each "issue" is one comic page, not
			// a chapter. Roll the entire archive into a single Chapter so the
			// Aidoku reader scrolls through all pages at once.
			let chapter = Chapter {
				key: "all".to_string(),
				title: Some("Все страницы".to_string()),
				chapter_number: Some(1.0),
				url: Some(format!("{SITE_URL}/~{slug}")),
				..Default::default()
			};
			updated.chapters = Some(alloc::vec![chapter]);
		}

		Ok(updated)
	}

	fn get_page_list(&self, manga: Manga, _chapter: Chapter) -> Result<Vec<Page>> {
		let urls = collect_issue_urls(&manga.key)?;
		Ok(urls
			.into_iter()
			.map(|u| Page {
				content: PageContent::url(u),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for Acomics {
	fn get_manga_list(&self, _listing: Listing, page: i32) -> Result<MangaPageResult> {
		let skip = PAGE_SIZE * (page - 1).max(0);
		let url = format!("{SITE_URL}/comics?skip={skip}");
		let doc = fetch_html(&url)?;
		Ok(parse_catalog(&doc, SITE_URL))
	}
}

impl Home for Acomics {
	fn get_home(&self) -> Result<HomeLayout> {
		let url = format!("{SITE_URL}/comics");
		let doc = fetch_html(&url)?;
		let entries = parse_catalog(&doc, SITE_URL).entries;
		let big_entries: Vec<Manga> = entries.iter().take(5).cloned().collect();
		let scroller_entries: Vec<Link> = entries.into_iter().skip(5).map(Link::from).collect();
		Ok(HomeLayout {
			components: alloc::vec![
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
			],
		})
	}
}

impl ImageRequestProvider for Acomics {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?
			.header(
				"User-Agent",
				"Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
			)
			.header("Referer", SITE_URL))
	}
}

impl DeepLinkHandler for Acomics {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if let Some(key) = url_to_key(&url) {
			return Ok(Some(DeepLinkResult::Manga { key }));
		}
		Ok(None)
	}
}

register_source!(
	Acomics,
	ListingProvider,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
