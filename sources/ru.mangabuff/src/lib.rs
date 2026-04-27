#![no_std]
extern crate alloc;

use aidoku::helpers::uri::encode_uri_component;
use aidoku::imports::html::{Document, Element, Html};
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

const SITE_URL: &str = "https://mangabuff.ru";

fn fetch_html(url: &str) -> Result<Document> {
	let response = Request::get(url)?
		.header(
			"User-Agent",
			"Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
		)
		.header("Referer", SITE_URL)
		.header("Accept", "text/html,*/*;q=0.8")
		.header("Accept-Language", "ru,en;q=0.9")
		.send()?;
	let status = response.status_code();
	let bytes = response.get_data()?;
	if !(200..400).contains(&status) {
		return Err(error!("MangaBuff HTTP {status} for {url}"));
	}
	Html::parse_with_url(bytes, SITE_URL).map_err(|e| error!("MangaBuff parse: {:?}", e))
}

/// Catalog tile = `<a class="cards__item" href="…/manga/{slug}">` with
/// `cards__name`, `cards__info` text and a `cards__img` whose
/// `style="background-image: url('/x180/img/manga/posters/{slug}.jpg')"` is
/// the cover.
fn parse_tile(el: &Element) -> Option<Manga> {
	let href = el.attr("abs:href").or_else(|| el.attr("href"))?;
	let key = url_to_key(&href)?;
	let title = el
		.select_first("div.cards__name")
		.and_then(|e| e.text())
		.filter(|s| !s.is_empty())?;
	let cover = el
		.select_first("div.cards__img")
		.and_then(|e| e.attr("style"))
		.as_deref()
		.and_then(extract_background_image)
		.map(|u| absolutize(&u));
	let genres = el
		.select_first("div.cards__info")
		.and_then(|e| e.text())
		.filter(|s| !s.is_empty());
	let tags = genres.map(|g| {
		g.split(',')
			.map(|s| s.trim().to_string())
			.filter(|s| !s.is_empty())
			.collect::<Vec<_>>()
	});
	Some(Manga {
		key,
		title,
		cover,
		url: Some(absolutize(&href)),
		tags,
		..Default::default()
	})
}

fn parse_catalog(doc: &Document) -> MangaPageResult {
	let entries = doc
		.select("a.cards__item")
		.map(|list| list.into_iter().filter_map(|el| parse_tile(&el)).collect::<Vec<_>>())
		.unwrap_or_default();
	// Pagination on /manga uses ?page=N. Detect "next page" link in the
	// pagination block. Be permissive — if any link with rel=next or
	// containing ?page=N+1 is present, assume there's more.
	let has_next_page = doc.select_first("a[rel=\"next\"]").is_some()
		|| doc
			.select(".pagination a")
			.map(|list| {
				list.into_iter()
					.any(|a| a.attr("href").map(|h| h.contains("?page=")).unwrap_or(false))
			})
			.unwrap_or(false);
	MangaPageResult {
		entries,
		has_next_page,
	}
}

fn url_to_key(url: &str) -> Option<String> {
	let after = url.split("/manga/").nth(1)?;
	let key = after.split('/').next()?;
	if key.is_empty() {
		return None;
	}
	Some(key.to_string())
}

fn absolutize(url: &str) -> String {
	if url.starts_with("http://") || url.starts_with("https://") {
		url.to_string()
	} else if let Some(p) = url.strip_prefix("//") {
		format!("https://{p}")
	} else if url.starts_with('/') {
		format!("{SITE_URL}{url}")
	} else {
		format!("{SITE_URL}/{url}")
	}
}

fn extract_background_image(style: &str) -> Option<String> {
	let needle = "url(";
	let start = style.find(needle)? + needle.len();
	let rest = &style[start..];
	let end = rest.find(')')?;
	let mut url = rest[..end].trim().to_string();
	if (url.starts_with('"') && url.ends_with('"'))
		|| (url.starts_with('\'') && url.ends_with('\''))
	{
		url = url[1..url.len() - 1].to_string();
	}
	if url.is_empty() { None } else { Some(url) }
}

fn fill_details(doc: &Document, manga: &mut Manga) {
	if let Some(t) = doc
		.select_first("h1.manga__title, .manga__name, h1")
		.and_then(|e| e.text())
		.filter(|s| !s.is_empty())
	{
		manga.title = t;
	}
	let cover = doc
		.select_first("meta[property=\"og:image\"]")
		.and_then(|e| e.attr("content"))
		.filter(|s| !s.is_empty());
	if cover.is_some() {
		manga.cover = cover;
	}
	manga.description = doc
		.select_first("meta[property=\"og:description\"]")
		.or_else(|| doc.select_first("meta[name=\"description\"]"))
		.and_then(|e| e.attr("content"))
		.filter(|s| !s.is_empty());

	let mut tags: Vec<String> = Vec::new();
	if let Some(genres) = doc.select(".manga__genres a, .info__list-text a[href*='?genres=']") {
		for g in genres {
			if let Some(t) = g.text() {
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
	manga.content_rating = ContentRating::Safe;
	manga.status = MangaStatus::Unknown;
}

/// Chapter row: `<a class="chapters__item" href="/manga/{slug}/{vol}/{chap}"
/// data-chapter="N" data-chapter-date="DD.MM.YYYY">`. Volume in
/// `.chapters__volume span`, chapter number in `.chapters__value span` (also
/// duplicated in data-chapter), name in `.chapters__name`.
fn parse_chapter_row(el: &Element) -> Option<Chapter> {
	let href = el.attr("abs:href").or_else(|| el.attr("href"))?;
	let chapter_number = el
		.attr("data-chapter")
		.and_then(|s| s.parse::<f32>().ok());
	let volume_number = el
		.select_first("div.chapters__volume span")
		.and_then(|e| e.text())
		.and_then(|s| s.trim().parse::<f32>().ok());
	let date_uploaded = el
		.attr("data-chapter-date")
		.as_deref()
		.and_then(parse_dot_date);
	let title = el
		.select_first("div.chapters__name")
		.and_then(|e| e.text())
		.map(|s| s.trim().to_string())
		.filter(|s| !s.is_empty());

	// Key = path without host, e.g. "/manga/slug/1/99".
	let key = strip_host(&href);

	Some(Chapter {
		key,
		title,
		chapter_number,
		volume_number,
		date_uploaded,
		url: Some(absolutize(&href)),
		..Default::default()
	})
}

fn strip_host(url: &str) -> String {
	if let Some(idx) = url.find("://") {
		let after = &url[idx + 3..];
		match after.find('/') {
			Some(slash) => after[slash..].to_string(),
			None => "/".to_string(),
		}
	} else {
		url.to_string()
	}
}

/// "24.04.2026" -> unix seconds (UTC midnight).
fn parse_dot_date(s: &str) -> Option<i64> {
	let parts: Vec<&str> = s.trim().split('.').collect();
	if parts.len() != 3 {
		return None;
	}
	let day: i64 = parts[0].parse().ok()?;
	let month: i64 = parts[1].parse().ok()?;
	let year: i64 = parts[2].parse().ok()?;
	let days = days_from_civil(year, month as u32, day as u32);
	Some(days * 86400)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
	let y = if m <= 2 { y - 1 } else { y };
	let era = if y >= 0 { y } else { y - 399 } / 400;
	let yoe = (y - era * 400) as u64;
	let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) as u64 + 2) / 5 + d as u64 - 1;
	let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
	era * 146097 + doe as i64 - 719468
}

struct MangaBuff;

impl Source for MangaBuff {
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
		let url = if let Some(q) = query.as_ref().filter(|q| !q.trim().is_empty()) {
			format!(
				"{SITE_URL}/search?query={}&page={page}",
				encode_uri_component(q.as_str())
			)
		} else {
			format!("{SITE_URL}/manga?page={page}")
		};
		let doc = fetch_html(&url)?;
		Ok(parse_catalog(&doc))
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
			let url = format!("{SITE_URL}/manga/{slug}");
			let doc = fetch_html(&url)?;
			updated.url = Some(url);
			if needs_details {
				fill_details(&doc, &mut updated);
			}
			if needs_chapters {
				let chapters = doc
					.select("a.chapters__item")
					.map(|list| {
						list.into_iter()
							.filter_map(|el| parse_chapter_row(&el))
							.collect::<Vec<_>>()
					})
					.unwrap_or_default();
				updated.chapters = Some(chapters);
			}
		}

		Ok(updated)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = if chapter.key.starts_with("http") {
			chapter.key.clone()
		} else {
			absolutize(&chapter.key)
		};
		let doc = fetch_html(&url)?;
		// Reader items lazy-load via <img data-src="https://c3.mangabuff.ru/...">.
		let urls = doc
			.select("div.reader__item img")
			.map(|list| {
				list.into_iter()
					.filter_map(|img| {
						img.attr("abs:data-src")
							.or_else(|| img.attr("data-src"))
							.or_else(|| img.attr("abs:src"))
							.or_else(|| img.attr("src"))
					})
					.filter(|s| !s.is_empty() && s.contains("/chapters/"))
					.collect::<Vec<_>>()
			})
			.unwrap_or_default();
		Ok(urls
			.into_iter()
			.map(|u| Page {
				content: PageContent::url(u),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for MangaBuff {
	fn get_manga_list(&self, _listing: Listing, page: i32) -> Result<MangaPageResult> {
		let url = format!("{SITE_URL}/manga?page={page}");
		let doc = fetch_html(&url)?;
		Ok(parse_catalog(&doc))
	}
}

impl Home for MangaBuff {
	fn get_home(&self) -> Result<HomeLayout> {
		let doc = fetch_html(&format!("{SITE_URL}/manga"))?;
		let entries = parse_catalog(&doc).entries;
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

impl ImageRequestProvider for MangaBuff {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?
			.header(
				"User-Agent",
				"Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
			)
			.header("Referer", SITE_URL))
	}
}

impl DeepLinkHandler for MangaBuff {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if let Some(key) = url_to_key(&url) {
			return Ok(Some(DeepLinkResult::Manga { key }));
		}
		Ok(None)
	}
}

register_source!(
	MangaBuff,
	ListingProvider,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
