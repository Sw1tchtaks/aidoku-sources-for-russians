#![no_std]
extern crate alloc;

use aidoku::helpers::uri::encode_uri_component;
use aidoku::imports::defaults::defaults_get;
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

const DEFAULT_BASE_URL: &str = "https://mangabuff.ru";
const SITE_ORIGINS: [&str; 3] = [
	"https://mangabuff.ru",
	"https://wss.mangabuff.ru",
	"https://wss2.mangabuff.ru",
];

fn base_url() -> String {
	let mut url = defaults_get::<String>("baseUrl")
		.unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
	while url.ends_with('/') {
		url.pop();
	}
	url
}

fn high_quality_covers() -> bool {
	defaults_get::<bool>("highQualityCovers").unwrap_or(true)
}

fn site_url(path: &str) -> String {
	let base = base_url();
	if path.starts_with('/') {
		format!("{base}{path}")
	} else {
		format!("{base}/{path}")
	}
}

fn fetch_html(url: &str) -> Result<Document> {
	let base = base_url();
	let response = Request::get(url)?
		.header(
			"User-Agent",
			"Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
		)
		.header("Referer", &base)
		.header("Accept", "text/html,*/*;q=0.8")
		.header("Accept-Language", "ru,en;q=0.9")
		.send()?;
	let status = response.status_code();
	let bytes = response.get_data()?;
	if !(200..400).contains(&status) {
		return Err(error!("MangaBuff HTTP {status} for {url}"));
	}
	Html::parse_with_url(bytes, &base).map_err(|e| error!("MangaBuff parse: {:?}", e))
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
		.map(|u| poster_url(&u));
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
	let rating = el
		.select_first("span.cards__rating")
		.and_then(|e| e.text())
		.map(|s| s.trim().to_string())
		.filter(|s| !s.is_empty());
	let authors = card_subtitle(rating.as_deref(), tags.as_deref());
	Some(Manga {
		key,
		title,
		cover,
		url: Some(absolutize(&href)),
		tags,
		authors,
		..Default::default()
	})
}

fn card_subtitle(rating: Option<&str>, tags: Option<&[String]>) -> Option<Vec<String>> {
	let mut parts = Vec::new();
	if let Some(value) = rating {
		parts.push(format!("★ {value}"));
	}
	if let Some(values) = tags {
		parts.extend(values.iter().take(2).cloned());
	}
	if parts.is_empty() {
		None
	} else {
		Some(alloc::vec![parts.join(" • ")])
	}
}

fn parse_tiles(el: &Element) -> Vec<Manga> {
	el.select("a.cards__item")
		.map(|list| {
			list.into_iter()
				.filter_map(|item| parse_tile(&item))
				.collect::<Vec<_>>()
		})
		.unwrap_or_default()
}

fn parse_catalog(doc: &Document) -> MangaPageResult {
	let entries = doc
		.select("a.cards__item")
		.map(|list| list.into_iter().filter_map(|el| parse_tile(&el)).collect::<Vec<_>>())
		.unwrap_or_default();
	// MangaBuff marks the current pagination item, so the following item is
	// the next page. This also avoids treating the last page as non-final.
	let has_next_page = doc.select_first("a[rel=\"next\"]").is_some()
		|| doc
			.select_first(".pagination__button--active + .pagination__button a")
			.is_some();
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
	for origin in SITE_ORIGINS {
		if url == origin {
			return base_url();
		}
		if let Some(path) = url.strip_prefix(origin).filter(|path| path.starts_with('/')) {
			return site_url(path);
		}
	}
	if let Some(path) = url.strip_prefix("//mangabuff.ru") {
		return site_url(path);
	}
	if let Some(path) = url.strip_prefix("//wss.mangabuff.ru") {
		return site_url(path);
	}
	if let Some(path) = url.strip_prefix("//wss2.mangabuff.ru") {
		return site_url(path);
	}
	if url.starts_with("http://") || url.starts_with("https://") {
		url.to_string()
	} else if let Some(p) = url.strip_prefix("//") {
		format!("https://{p}")
	} else if url.starts_with('/') {
		site_url(url)
	} else {
		site_url(url)
	}
}

fn poster_url(url: &str) -> String {
	let full_marker = "/img/manga/posters/";
	let thumbnail_marker = "/x180/img/manga/posters/";
	let path = if let Some(index) = url.find(thumbnail_marker) {
		Some(&url[index + "/x180".len()..])
	} else {
		url.find(full_marker).map(|index| &url[index..])
	};
	let Some(path) = path else {
		return absolutize(url);
	};
	if high_quality_covers() {
		format!("{DEFAULT_BASE_URL}{path}")
	} else {
		site_url(&format!("/x180{path}"))
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
	if let Some(cover) = cover {
		manga.cover = Some(poster_url(&cover));
	}
	manga.description = doc
		.select_first("meta[property=\"og:description\"]")
		.or_else(|| doc.select_first("meta[name=\"description\"]"))
		.and_then(|e| e.attr("content"))
		.filter(|s| !s.is_empty());

	// The current MangaBuff layout keeps age rating, genres and descriptive
	// tags together in `.manga__middle .tags`. Preserve the type/primary genre
	// from the catalog tile, then append every unique detail tag. The page has
	// duplicate desktop/mobile blocks, hence the explicit de-duplication.
	let mut tags = manga.tags.clone().unwrap_or_default();
	if let Some(detail_tags) = doc.select(".manga__middle .tags .tags__item") {
		for tag in detail_tags {
			if let Some(text) = tag.text() {
				let value = text.trim();
				if !value.is_empty() && !tags.iter().any(|existing| existing == value) {
					tags.push(value.to_string());
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
				"{}/search?query={}&page={page}",
				base_url(),
				encode_uri_component(q.as_str())
			)
		} else {
			format!("{}/manga?page={page}", base_url())
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
			let url = site_url(&format!("/manga/{slug}"));
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
		let url = absolutize(&chapter.key);
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
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		if matches!(listing.id.as_str(), "hot" | "latest") {
			if page > 1 {
				return Ok(MangaPageResult {
					entries: Vec::new(),
					has_next_page: false,
				});
			}
			let doc = fetch_html(&base_url())?;
			let index = if listing.id == "hot" { 1 } else { 2 };
			return Ok(MangaPageResult {
				entries: parse_home_section(&doc, index),
				has_next_page: false,
			});
		}
		let path = match listing.id.as_str() {
			"manga" => "/types/manga",
			"manhwa" => "/types/manxva",
			"manhua" => "/types/manxya",
			_ => "/manga",
		};
		let url = format!("{}{path}?page={page}", base_url());
		let doc = fetch_html(&url)?;
		Ok(parse_catalog(&doc))
	}
}

impl Home for MangaBuff {
	fn get_home(&self) -> Result<HomeLayout> {
		let doc = fetch_html(&base_url())?;
		let popular = parse_home_section(&doc, 0);
		let mut hot = parse_home_section(&doc, 1);
		let mut latest = parse_home_section(&doc, 2);

		let mut featured: Vec<Manga> = popular.iter().take(3).cloned().collect();
		enrich_entries(&mut featured, 3);
		// MangaBuff exposes only type + primary genre in home-card HTML. Fetch
		// details for the six rows Aidoku actually renders in each MangaList so
		// those cards carry the complete tag set without enriching all 20 items.
		enrich_entries(&mut hot, 6);
		enrich_entries(&mut latest, 6);

		let manga = home_catalog("/types/manga").unwrap_or_default();
		let manhwa = home_catalog("/types/manxva").unwrap_or_default();
		let manhua = home_catalog("/types/manxya").unwrap_or_default();

		Ok(HomeLayout {
			components: alloc::vec![
				HomeComponent {
					title: Some("Популярное".to_string()),
					subtitle: Some("Сейчас на MangaBuff".to_string()),
					value: HomeComponentValue::BigScroller {
						entries: featured,
						auto_scroll_interval: Some(8.0),
					},
				},
				HomeComponent {
					title: Some("Горячие новинки".to_string()),
					subtitle: Some("Рейтинг, тип и жанр".to_string()),
					value: HomeComponentValue::MangaList {
						ranking: false,
						page_size: Some(6),
						entries: hot.into_iter().take(6).map(Link::from).collect(),
						listing: Some(listing("hot", "Горячие новинки")),
					},
				},
				HomeComponent {
					title: Some("Новое на сайте".to_string()),
					subtitle: Some("Недавно добавленные тайтлы".to_string()),
					value: HomeComponentValue::MangaList {
						ranking: false,
						page_size: Some(6),
						entries: latest.into_iter().take(6).map(Link::from).collect(),
						listing: Some(listing("latest", "Новое на сайте")),
					},
				},
				catalog_scroller("Манга", "Японские тайтлы", manga, "manga"),
				catalog_scroller("Манхва", "Корейские тайтлы", manhwa, "manhwa"),
				catalog_scroller("Маньхуа", "Китайские тайтлы", manhua, "manhua"),
				HomeComponent {
					title: Some("Весь каталог".to_string()),
					subtitle: Some("Все доступные тайтлы".to_string()),
					value: HomeComponentValue::Scroller {
						entries: popular.into_iter().skip(3).map(Link::from).collect(),
						listing: Some(listing("catalog", "Каталог")),
					},
				},
			],
		})
	}
}

fn enrich_entries(entries: &mut [Manga], limit: usize) {
	for manga in entries.iter_mut().take(limit) {
		if let Ok(details) = fetch_html(&site_url(&format!("/manga/{}", manga.key))) {
			fill_details(&details, manga);
		}
	}
}

fn parse_home_section(doc: &Document, index: usize) -> Vec<Manga> {
	doc.select("div.cards.owl-carousel")
		.and_then(|list| list.into_iter().nth(index))
		.map(|section| parse_tiles(&section))
		.unwrap_or_default()
}

fn home_catalog(path: &str) -> Result<Vec<Manga>> {
	let doc = fetch_html(&site_url(path))?;
	Ok(parse_catalog(&doc).entries.into_iter().take(12).collect())
}

fn listing(id: &str, name: &str) -> Listing {
	Listing {
		id: id.to_string(),
		name: name.to_string(),
		kind: ListingKind::Default,
	}
}

fn catalog_scroller(
	title: &str,
	subtitle: &str,
	entries: Vec<Manga>,
	listing_id: &str,
) -> HomeComponent {
	HomeComponent {
		title: Some(title.to_string()),
		subtitle: Some(subtitle.to_string()),
		value: HomeComponentValue::Scroller {
			entries: entries.into_iter().map(Link::from).collect(),
			listing: Some(listing(listing_id, title)),
		},
	}
}

impl ImageRequestProvider for MangaBuff {
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
