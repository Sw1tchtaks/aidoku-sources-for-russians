#![no_std]
extern crate alloc;

use aidoku::helpers::uri::encode_uri_component;
use aidoku::imports::defaults::{DefaultValue, defaults_get, defaults_set};
use aidoku::imports::net::{Request, TimeUnit, set_rate_limit};
use aidoku::prelude::*;
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, HashMap, Home,
	HomeComponent, HomeComponentValue, HomeLayout, ImageRequestProvider, Link, Listing,
	ListingKind, ListingProvider, Manga, MangaPageResult, MangaStatus, Page, PageContent,
	PageContext, Result, Source, Viewer, WebLoginHandler,
	alloc::{String, Vec},
};
use alloc::format;
use alloc::string::ToString;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

const DEFAULT_BASE_URL: &str = "https://inkstory.net";
const DEFAULT_API_URL: &str = "https://api.inkstory.net";
const STORED_COOKIE_KEY: &str = "inkstory.cookie";
const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";
const PAGE_SIZE: usize = 20;
const HOME_FEATURED_COUNT: usize = 3;
const HOME_SCROLLER_COUNT: usize = 18;

#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Book {
	id: Option<String>,
	slug: String,
	poster: Option<String>,
	status: Option<String>,
	content_status: Option<String>,
	name: LocalizedName,
	description: Option<String>,
	labels: Option<Vec<Label>>,
	country: Option<String>,
	formats: Option<Vec<String>>,
	chapters_count: Option<i32>,
	average_rating: Option<f32>,
}

#[derive(Clone, Default, Deserialize)]
struct LocalizedName {
	ru: Option<String>,
	en: Option<String>,
	original: Option<String>,
}

#[derive(Clone, Default, Deserialize)]
struct Label {
	name: Option<String>,
	kind: Option<String>,
}

#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FeedItem {
	book: Book,
	chapters: Vec<ApiChapter>,
}

#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiChapter {
	id: String,
	name: Option<String>,
	title: Option<String>,
	number: Option<f32>,
	volume: Option<f32>,
	branch_id: Option<String>,
	created_at: Option<String>,
	updated_at: Option<String>,
	formated_date: Option<String>,
}

#[derive(Default, Deserialize)]
struct Branch {
	id: String,
	publishers: Option<Vec<Publisher>>,
}

#[derive(Default, Deserialize)]
struct Publisher {
	name: Option<String>,
}

#[derive(Default, Deserialize)]
struct ChapterDetail {
	pages: Vec<PageImage>,
}

#[derive(Default, Deserialize)]
struct PageImage {
	image: String,
	index: Option<i32>,
}

struct InkStory;

impl Source for InkStory {
	fn new() -> Self {
		set_rate_limit(3, 1, TimeUnit::Seconds);
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if let Some(q) = query.as_ref().map(|q| q.trim()).filter(|q| !q.is_empty()) {
			let url = api_url(&format!(
				"/v2/books?search={}&page={}",
				encode_uri_component(q),
				api_page(page)
			));
			return books_to_result(fetch_json::<Vec<Book>>(&url)?, page);
		}
		fetch_sorted_books("viewsWeekCount,desc", None, page)
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
			let book = fetch_json::<Book>(&api_url(&format!(
				"/v2/books/{}",
				encode_uri_component(&slug)
			)))?;
			updated = book_to_manga(book);
		}

		if needs_chapters {
			let html = fetch_text(&site_url(&format!("/content/{slug}")))?;
			let store = extract_astro_store(&html)?;
			let chapters = store
				.get("current-book-chapters")
				.cloned()
				.and_then(|value| serde_json::from_value::<Vec<ApiChapter>>(value).ok())
				.unwrap_or_default();
			let branches = store
				.get("current-book-branches")
				.cloned()
				.and_then(|value| serde_json::from_value::<Vec<Branch>>(value).ok())
				.unwrap_or_default();
			updated.chapters = Some(
				chapters
					.into_iter()
					.map(|c| chapter_to_chapter(c, &slug, &branches))
					.collect(),
			);
		}

		Ok(updated)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let mut pages = fetch_json::<ChapterDetail>(&api_url(&format!(
			"/v2/chapters/{}",
			encode_uri_component(&chapter.key)
		)))?
		.pages;
		pages.sort_by_key(|p| p.index.unwrap_or(0));
		Ok(pages
			.into_iter()
			.map(|p| Page {
				content: PageContent::url(p.image),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for InkStory {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		match listing.id.as_str() {
			"popular_day" => fetch_sorted_books("viewsDayCount,desc", None, page),
			"popular_month" => fetch_sorted_books("viewsMonthCount,desc", None, page),
			"popular_all" => fetch_sorted_books("viewsCount,desc", None, page),
			"latest_updates" => fetch_update_feed(page),
			"latest_titles" => fetch_sorted_books("createdAt,desc", None, page),
			"manga" => fetch_sorted_books("viewsWeekCount,desc", Some("JAPAN"), page),
			"manhwa" => fetch_sorted_books("viewsWeekCount,desc", Some("KOREA"), page),
			"manhua" => fetch_sorted_books("viewsWeekCount,desc", Some("CHINA"), page),
			_ => fetch_sorted_books("viewsWeekCount,desc", None, page),
		}
	}
}

impl Home for InkStory {
	fn get_home(&self) -> Result<HomeLayout> {
		let day = fetch_book_vec("viewsDayCount,desc", None, 1).unwrap_or_default();
		let week = fetch_book_vec("viewsWeekCount,desc", None, 1).unwrap_or_default();
		let updates = fetch_update_vec(1).unwrap_or_default();
		let latest = fetch_book_vec("createdAt,desc", None, 1).unwrap_or_default();
		let manga = fetch_book_vec("createdAt,desc", Some("JAPAN"), 1).unwrap_or_default();
		let manhwa = fetch_book_vec("updatedAt,desc", Some("KOREA"), 1).unwrap_or_default();
		let manhua = fetch_book_vec("updatedAt,desc", Some("CHINA"), 1).unwrap_or_default();

		let featured: Vec<Manga> = day
			.iter()
			.take(HOME_FEATURED_COUNT)
			.cloned()
			.map(book_to_manga)
			.map(|m| self.get_manga_update(m.clone(), true, false).unwrap_or(m))
			.collect();
		let more_day: Vec<Manga> = day
			.into_iter()
			.skip(HOME_FEATURED_COUNT)
			.take(HOME_SCROLLER_COUNT)
			.map(book_to_manga)
			.collect();

		let mut components = Vec::with_capacity(8);
		if !featured.is_empty() {
			components.push(HomeComponent {
				title: Some("Популярное".to_string()),
				subtitle: Some("За день".to_string()),
				value: HomeComponentValue::BigScroller {
					entries: featured,
					auto_scroll_interval: Some(8.0),
				},
			});
		}
		push_scroller(
			&mut components,
			"popular_day",
			"Ещё популярное",
			"Продолжение дневного топа",
			more_day,
		);
		push_scroller(
			&mut components,
			"popular_week",
			"Читают за неделю",
			"Живой недельный топ",
			week.into_iter().map(book_to_manga).collect(),
		);
		push_scroller(
			&mut components,
			"latest_updates",
			"Последние обновления",
			"Свежие главы",
			updates,
		);
		push_scroller(
			&mut components,
			"latest_titles",
			"Новинки",
			"Недавно добавленные тайтлы",
			latest.into_iter().map(book_to_manga).collect(),
		);
		push_scroller(
			&mut components,
			"manga",
			"Манга",
			"Японские тайтлы",
			manga.into_iter().map(book_to_manga).collect(),
		);
		push_scroller(
			&mut components,
			"manhwa",
			"Манхва",
			"Корейские тайтлы",
			manhwa.into_iter().map(book_to_manga).collect(),
		);
		push_scroller(
			&mut components,
			"manhua",
			"Маньхуа",
			"Китайские тайтлы",
			manhua.into_iter().map(book_to_manga).collect(),
		);
		Ok(HomeLayout { components })
	}
}

impl ImageRequestProvider for InkStory {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		let mut req = build_request(&url, "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8")?;
		req = req.header("Referer", &base_url());
		Ok(req)
	}
}

impl WebLoginHandler for InkStory {
	fn handle_web_login(&self, _key: String, cookies: HashMap<String, String>) -> Result<bool> {
		if cookies.is_empty() {
			return Ok(false);
		}
		let has_login_marker = cookies.iter().any(|(name, value)| {
			let lower = name.to_lowercase();
			lower.contains("auth")
				|| lower.contains("token")
				|| lower.contains("session")
				|| lower.contains("access")
				|| lower.contains("refresh")
				|| lower.contains("user")
				|| value.starts_with("eyJ")
		});
		if !has_login_marker && cookies.len() < 4 {
			println!(
				"[inkstory] login: {} cookies, no auth marker yet; keeping WebView open",
				cookies.len()
			);
			return Ok(false);
		}

		let mut header = String::new();
		for (k, v) in cookies.iter() {
			if !header.is_empty() {
				header.push_str("; ");
			}
			header.push_str(k);
			header.push('=');
			header.push_str(v);
		}
		defaults_set(STORED_COOKIE_KEY, DefaultValue::String(header));
		println!("[inkstory] login: stored {} cookies", cookies.len());
		Ok(true)
	}
}

impl DeepLinkHandler for InkStory {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if let Some(slug) = content_slug(&url) {
			return Ok(Some(DeepLinkResult::Manga { key: slug }));
		}
		Ok(None)
	}
}

fn base_url() -> String {
	trim_url(defaults_get::<String>("baseUrl").unwrap_or_else(|| DEFAULT_BASE_URL.to_string()))
}

fn api_base_url() -> String {
	trim_url(defaults_get::<String>("apiUrl").unwrap_or_else(|| DEFAULT_API_URL.to_string()))
}

fn trim_url(mut url: String) -> String {
	while url.ends_with('/') {
		url.pop();
	}
	url
}

fn api_url(path: &str) -> String {
	format!("{}{}", api_base_url(), path)
}

fn site_url(path: &str) -> String {
	format!("{}{}", base_url(), path)
}

fn api_page(page: i32) -> i32 {
	(page - 1).max(0)
}

fn stored_cookie() -> Option<String> {
	defaults_get::<String>(STORED_COOKIE_KEY).filter(|s| !s.trim().is_empty())
}

fn manual_cookies() -> Option<String> {
	defaults_get::<String>("manualCookies").filter(|s| !s.trim().is_empty())
}

fn auth_token() -> Option<String> {
	defaults_get::<String>("authToken").filter(|s| !s.trim().is_empty())
}

fn build_request(url: &str, accept: &str) -> Result<Request> {
	let base = base_url();
	let mut req = Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Accept", accept)
		.header("Accept-Language", "ru,en;q=0.9")
		.header("Referer", &base)
		.header("Origin", &base);
	match (manual_cookies(), stored_cookie()) {
		(Some(m), Some(c)) => req = req.header("Cookie", &format!("{m}; {c}")),
		(Some(m), None) => req = req.header("Cookie", &m),
		(None, Some(c)) => req = req.header("Cookie", &c),
		(None, None) => {}
	}
	if let Some(token) = auth_token() {
		req = req.header("Authorization", &format!("Bearer {token}"));
	}
	Ok(req)
}

fn fetch_text(url: &str) -> Result<String> {
	let response = build_request(url, "text/html,application/xhtml+xml,application/json;q=0.9,*/*;q=0.8")?.send()?;
	let status = response.status_code();
	let bytes = response.get_data()?;
	if !(200..400).contains(&status) {
		return Err(error!("InkStory HTTP {status} for {url}"));
	}
	Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn fetch_json<T: DeserializeOwned>(url: &str) -> Result<T> {
	let response = build_request(url, "application/json,text/plain,*/*")?.send()?;
	let status = response.status_code();
	let bytes = response.get_data()?;
	if !(200..400).contains(&status) {
		let preview = preview(&bytes);
		return Err(error!("InkStory HTTP {status} for {url}: {preview}"));
	}
	serde_json::from_slice(&bytes).map_err(|e| {
		let preview = preview(&bytes);
		error!("InkStory decode {url}: {e}; body: {preview}")
	})
}

fn preview(bytes: &[u8]) -> String {
	let n = bytes.len().min(240);
	String::from_utf8_lossy(&bytes[..n]).into_owned()
}

fn fetch_book_vec(sort: &str, country: Option<&str>, page: i32) -> Result<Vec<Book>> {
	let mut url = format!(
		"/v2/books?sort={}&page={}",
		encode_uri_component(sort),
		api_page(page)
	);
	if let Some(country) = country {
		url.push_str("&country=");
		url.push_str(country);
	}
	fetch_json(&api_url(&url))
}

fn fetch_sorted_books(sort: &str, country: Option<&str>, page: i32) -> Result<MangaPageResult> {
	books_to_result(fetch_book_vec(sort, country, page)?, page)
}

fn fetch_update_vec(page: i32) -> Result<Vec<Manga>> {
	let items = fetch_update_items(page)?;
	Ok(items.into_iter().map(feed_item_to_manga).collect())
}

fn fetch_update_feed(page: i32) -> Result<MangaPageResult> {
	let items = fetch_update_items(page)?;
	let entries = items.into_iter().map(feed_item_to_manga).collect::<Vec<_>>();
	Ok(MangaPageResult {
		has_next_page: entries.len() >= PAGE_SIZE,
		entries,
	})
}

fn fetch_update_items(page: i32) -> Result<Vec<FeedItem>> {
	fetch_json(&api_url(&format!(
		"/v2/chapter-update-feed?onlyBorderChapters=true&sort=updatedAt%2Cdesc&page={}",
		api_page(page)
	)))
}

fn books_to_result(books: Vec<Book>, _page: i32) -> Result<MangaPageResult> {
	let has_next_page = books.len() >= PAGE_SIZE;
	Ok(MangaPageResult {
		entries: books.into_iter().map(book_to_manga).collect(),
		has_next_page,
	})
}

fn book_to_manga(book: Book) -> Manga {
	let title = best_name(&book.name).unwrap_or_else(|| book.slug.clone());
	let viewer = viewer_for(&book);
	let mut tags = Vec::new();
	if let Some(labels) = book.labels {
		for label in labels {
			if label.kind.as_deref() == Some("ADULT") {
				continue;
			}
			if let Some(name) = label.name.filter(|s| !s.trim().is_empty()) {
				tags.push(name);
			}
		}
	}
	if let Some(country) = book.country.as_deref().and_then(country_label) {
		tags.push(country.to_string());
	}
	if let Some(count) = book.chapters_count.filter(|c| *c > 0) {
		tags.push(format!("{count} глав"));
	}
	if let Some(rating) = book.average_rating {
		if rating > 0.0 {
			tags.push(format!("{rating:.2}"));
		}
	}
	Manga {
		key: book.slug.clone(),
		title,
		cover: book.poster.map(|u| image_url(&u)),
		url: Some(site_url(&format!("/content/{}", book.slug))),
		description: book.description.filter(|s| !s.trim().is_empty()),
		status: map_status(book.status.as_deref()),
		content_rating: map_content_rating(book.content_status.as_deref()),
		viewer,
		tags: if tags.is_empty() { None } else { Some(tags) },
		..Default::default()
	}
}

fn feed_item_to_manga(item: FeedItem) -> Manga {
	let mut manga = book_to_manga(item.book);
	let latest = item.chapters.first().and_then(|c| {
		c.title
			.clone()
			.or_else(|| c.name.clone())
			.or_else(|| c.number.map(|n| format!("Глава {}", trim_number(n))))
	});
	let mut tags = manga.tags.take().unwrap_or_default();
	if let Some(latest) = latest {
		tags.insert(0, latest);
	}
	manga.tags = if tags.is_empty() { None } else { Some(tags) };
	manga
}

fn best_name(name: &LocalizedName) -> Option<String> {
	name.ru
		.as_ref()
		.or(name.en.as_ref())
		.or(name.original.as_ref())
		.map(|s| s.trim().to_string())
		.filter(|s| !s.is_empty())
}

fn country_label(country: &str) -> Option<&'static str> {
	match country {
		"JAPAN" => Some("Манга"),
		"KOREA" => Some("Манхва"),
		"CHINA" => Some("Маньхуа"),
		_ => None,
	}
}

fn viewer_for(book: &Book) -> Viewer {
	if book
		.formats
		.as_ref()
		.map(|formats| formats.iter().any(|f| f == "WEBTOON"))
		.unwrap_or(false)
	{
		return Viewer::Webtoon;
	}
	match book.country.as_deref() {
		Some("KOREA") | Some("CHINA") => Viewer::Webtoon,
		_ => Viewer::RightToLeft,
	}
}

fn map_status(status: Option<&str>) -> MangaStatus {
	match status {
		Some("ONGOING") | Some("ANNOUNCE") => MangaStatus::Ongoing,
		Some("FINISHED") | Some("COMPLETED") | Some("DONE") => MangaStatus::Completed,
		Some("HIATUS") | Some("PAUSED") | Some("FROZEN") => MangaStatus::Hiatus,
		Some("DROPPED") | Some("CANCELLED") => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}

fn map_content_rating(status: Option<&str>) -> ContentRating {
	match status {
		Some("PORNOGRAPHIC") | Some("EROTIC") | Some("EXPLICIT") | Some("ADULT") => {
			ContentRating::NSFW
		}
		Some("UNSAFE") | Some("SUGGESTIVE") | Some("QUESTIONABLE") => ContentRating::Suggestive,
		_ => ContentRating::Safe,
	}
}

fn image_url(url: &str) -> String {
	url.to_string()
}

fn chapter_to_chapter(chapter: ApiChapter, slug: &str, branches: &[Branch]) -> Chapter {
	let branch = chapter
		.branch_id
		.as_deref()
		.and_then(|id| branch_name(id, branches));
	let mut title = chapter
		.title
		.clone()
		.or_else(|| chapter.name.clone())
		.or_else(|| chapter.number.map(|n| format!("Глава {}", trim_number(n))));
	if let (Some(current), Some(branch)) = (title.as_mut(), branch) {
		if !current.contains(&branch) {
			current.push_str(" · ");
			current.push_str(&branch);
		}
	}
	Chapter {
		key: chapter.id.clone(),
		title,
		chapter_number: chapter.number,
		volume_number: chapter.volume,
		date_uploaded: chapter
			.created_at
			.as_deref()
			.or(chapter.updated_at.as_deref())
			.or(chapter.formated_date.as_deref())
			.and_then(parse_iso_datetime),
		url: Some(site_url(&format!("/content/{slug}/{}", chapter.id))),
		..Default::default()
	}
}

fn branch_name(id: &str, branches: &[Branch]) -> Option<String> {
	let branch = branches.iter().find(|b| b.id == id)?;
	let names = branch
		.publishers
		.as_ref()?
		.iter()
		.filter_map(|p| p.name.as_ref())
		.filter(|s| !s.trim().is_empty())
		.cloned()
		.collect::<Vec<_>>();
	if names.is_empty() {
		None
	} else {
		Some(names.join(", "))
	}
}

fn trim_number(n: f32) -> String {
	let whole = n as i32;
	if (n - whole as f32).abs() < 0.001 {
		format!("{whole}")
	} else {
		format!("{n}")
	}
}

fn parse_iso_datetime(s: &str) -> Option<i64> {
	if s.len() < 10 {
		return None;
	}
	let year: i64 = s.get(0..4)?.parse().ok()?;
	let month: u32 = s.get(5..7)?.parse().ok()?;
	let day: u32 = s.get(8..10)?.parse().ok()?;
	let hour: i64 = s.get(11..13).and_then(|v| v.parse().ok()).unwrap_or(0);
	let minute: i64 = s.get(14..16).and_then(|v| v.parse().ok()).unwrap_or(0);
	let second: i64 = s.get(17..19).and_then(|v| v.parse().ok()).unwrap_or(0);
	Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
	let y = if m <= 2 { y - 1 } else { y };
	let era = if y >= 0 { y } else { y - 399 } / 400;
	let yoe = (y - era * 400) as u64;
	let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) as u64 + 2) / 5 + d as u64 - 1;
	let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
	era * 146097 + doe as i64 - 719468
}

fn extract_astro_store(html: &str) -> Result<Map<String, Value>> {
	let marker = "id=\"it-astro-state\"";
	let marker_pos = html
		.find(marker)
		.ok_or(error!("InkStory: it-astro-state not found"))?;
	let after_marker = &html[marker_pos..];
	let start = after_marker
		.find('>')
		.ok_or(error!("InkStory: it-astro-state start not found"))?
		+ 1;
	let after_start = &after_marker[start..];
	let end = after_start
		.find("</script>")
		.ok_or(error!("InkStory: it-astro-state end not found"))?;
	let raw = after_start[..end].trim();
	let values: Vec<Value> =
		serde_json::from_str(raw).map_err(|e| error!("InkStory: state decode: {e}"))?;
	let decoded = decode_devalue_ref(&values, 0, 0);
	decoded
		.get("@inox-tools/request-nanostores")
		.and_then(|v| v.as_object())
		.cloned()
		.ok_or(error!("InkStory: request store not found"))
}

fn decode_devalue_ref(values: &[Value], index: i64, depth: usize) -> Value {
	if depth > 80 || index < 0 {
		return Value::Null;
	}
	let Some(raw) = values.get(index as usize) else {
		return Value::Null;
	};
	match raw {
		Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => raw.clone(),
		Value::Array(items) => decode_devalue_array(values, items, depth + 1),
		Value::Object(map) => {
			let mut out = Map::new();
			for (key, value) in map {
				out.insert(key.clone(), decode_devalue_value(values, value, depth + 1));
			}
			Value::Object(out)
		}
	}
}

fn decode_devalue_value(values: &[Value], value: &Value, depth: usize) -> Value {
	if let Some(index) = value.as_i64() {
		decode_devalue_ref(values, index, depth + 1)
	} else {
		value.clone()
	}
}

fn decode_devalue_array(values: &[Value], items: &[Value], depth: usize) -> Value {
	if let Some(Value::String(tag)) = items.first() {
		match tag.as_str() {
			"Map" => {
				let mut out = Map::new();
				let mut i = 1;
				while i + 1 < items.len() {
					let key = decode_devalue_value(values, &items[i], depth + 1);
					let value = decode_devalue_value(values, &items[i + 1], depth + 1);
					if let Some(key) = value_to_key(&key) {
						out.insert(key, value);
					}
					i += 2;
				}
				Value::Object(out)
			}
			"Set" => Value::Array(
				items
					.iter()
					.skip(1)
					.map(|v| decode_devalue_value(values, v, depth + 1))
					.collect(),
			),
			"Date" | "URL" | "BigInt" => items
				.get(1)
				.map(|v| decode_devalue_value(values, v, depth + 1))
				.unwrap_or(Value::Null),
			_ => Value::Null,
		}
	} else {
		Value::Array(
			items
				.iter()
				.map(|v| decode_devalue_value(values, v, depth + 1))
				.collect(),
		)
	}
}

fn value_to_key(value: &Value) -> Option<String> {
	match value {
		Value::String(s) => Some(s.clone()),
		Value::Number(n) => Some(n.to_string()),
		Value::Bool(b) => Some(b.to_string()),
		_ => None,
	}
}

fn push_scroller(
	components: &mut Vec<HomeComponent>,
	id: &'static str,
	title: &'static str,
	subtitle: &'static str,
	entries: Vec<Manga>,
) {
	if entries.is_empty() {
		return;
	}
	components.push(HomeComponent {
		title: Some(title.to_string()),
		subtitle: Some(subtitle.to_string()),
		value: HomeComponentValue::Scroller {
			entries: entries.into_iter().map(Link::from).collect(),
			listing: Some(Listing {
				id: id.to_string(),
				name: title.to_string(),
				kind: ListingKind::Default,
			}),
		},
	});
}

fn content_slug(url: &str) -> Option<String> {
	let after = url.split("/content/").nth(1)?;
	let slug = after.split('/').next()?.split('?').next()?.split('#').next()?;
	if slug.is_empty() {
		None
	} else {
		Some(slug.to_string())
	}
}

register_source!(
	InkStory,
	ListingProvider,
	Home,
	ImageRequestProvider,
	WebLoginHandler,
	DeepLinkHandler
);
