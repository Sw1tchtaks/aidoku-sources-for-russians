use aidoku::{Chapter, ContentRating, Manga, MangaStatus, Viewer};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::Deserialize;

pub const SITE_URL: &str = "https://xn--80ac9aeh6f.xn--p1ai";

// --- /v3/book ---

#[derive(Deserialize)]
pub struct CatalogEnvelope {
	pub items: Vec<CatalogItem>,
}

#[derive(Deserialize)]
pub struct CatalogItem {
	pub id: i64,
	pub title: String,
	#[serde(default)]
	pub description: Option<String>,
	pub slug: String,
	#[serde(default)]
	pub url: Option<String>,
	#[serde(default)]
	pub status: Option<String>,
	#[serde(default)]
	pub likes: Option<i64>,
	#[serde(default)]
	pub dislikes: Option<i64>,
}

impl CatalogItem {
	pub fn into_manga(self) -> Manga {
		// Cover is intentionally None in the catalog. The /v3/book endpoint
		// doesn't expose a stable cover URL, and the predictable
		// /images/books/{id}/vertical-NN.jpeg form depends on a per-book
		// upload number we can only get from the book page. The cover gets
		// filled in once the user opens the manga (BookFull.verticalImage.url).
		let url = self
			.url
			.clone()
			.unwrap_or_else(|| format!("{SITE_URL}/{}", self.slug));
		Manga {
			key: self.slug.clone(),
			title: self.title,
			url: Some(absolutize(&url)),
			status: parse_status(self.status.as_deref()),
			..Default::default()
		}
	}
}

fn absolutize(url: &str) -> String {
	if url.starts_with("http://") || url.starts_with("https://") {
		url.to_string()
	} else if url.starts_with('/') {
		format!("{SITE_URL}{url}")
	} else {
		format!("{SITE_URL}/{url}")
	}
}

fn parse_status(s: Option<&str>) -> MangaStatus {
	match s.unwrap_or("") {
		"ongoing" => MangaStatus::Ongoing,
		"completed" | "finished" | "translated" => MangaStatus::Completed,
		"abandoned" | "dropped" => MangaStatus::Cancelled,
		"frozen" | "hiatus" => MangaStatus::Hiatus,
		_ => MangaStatus::Unknown,
	}
}

// --- __NEXT_DATA__ for /{slug} (book page) ---

#[derive(Deserialize)]
pub struct NextData<T> {
	pub props: NextProps<T>,
}

#[derive(Deserialize)]
pub struct NextProps<T> {
	#[serde(rename = "pageProps")]
	pub page_props: T,
}

#[derive(Deserialize)]
pub struct BookPageProps {
	#[serde(rename = "initialReduxState", default)]
	pub initial_redux_state: Option<ReduxState>,
}

#[derive(Deserialize, Default)]
pub struct ReduxState {
	#[serde(default)]
	pub data: ReduxData,
}

#[derive(Deserialize, Default)]
pub struct ReduxData {
	#[serde(rename = "bookPage", default)]
	pub book_page: Option<BookPageData>,
}

// The book page in fact embeds book + chapters via a separate state slice.
// However the obvious top-level keys are minimal — the full chapters array
// lives in a sibling slice. Walk via serde_json::Value below.

#[derive(Deserialize, Default)]
pub struct BookPageData {
	#[serde(rename = "bookId", default)]
	pub book_id: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookFull {
	pub id: i64,
	pub title: String,
	pub slug: String,
	#[serde(default)]
	pub description: Option<String>,
	#[serde(default)]
	pub additional_info: Option<String>,
	#[serde(default)]
	pub status: Option<String>,
	#[serde(default)]
	pub author: Option<String>,
	#[serde(default)]
	pub genres: Option<Vec<NameRef>>,
	#[serde(default)]
	pub vertical_image: Option<ImageRef>,
	#[serde(default)]
	pub chapters: Option<Vec<ChapterDto>>,
}

#[derive(Deserialize)]
pub struct NameRef {
	#[serde(default)]
	pub title: Option<String>,
	#[serde(default)]
	pub name: Option<String>,
}

impl NameRef {
	pub fn label(&self) -> Option<String> {
		self.title
			.clone()
			.or_else(|| self.name.clone())
			.filter(|s| !s.is_empty())
	}
}

#[derive(Deserialize)]
pub struct ImageRef {
	#[serde(default)]
	pub url: Option<String>,
}

#[derive(Deserialize)]
pub struct ChapterDto {
	pub id: i64,
	#[serde(default)]
	pub title: Option<String>,
	#[serde(default)]
	pub url: Option<String>,
	#[serde(rename = "numberChapter", default)]
	pub number_chapter: Option<String>,
	#[serde(rename = "chapterShortNumber", default)]
	pub chapter_short_number: Option<f32>,
	#[serde(default)]
	pub tom: Option<i32>,
	#[serde(rename = "publishedAt", default)]
	pub published_at: Option<String>,
	#[serde(rename = "isDonate", default)]
	pub is_donate: bool,
	#[serde(rename = "isSubscription", default)]
	pub is_subscription: bool,
	#[serde(rename = "isUserPaid", default)]
	pub is_user_paid: bool,
}

impl ChapterDto {
	pub fn into_chapter(self) -> Chapter {
		let chapter_number = self.chapter_short_number;
		let volume_number = self.tom.map(|v| v as f32);
		let date_uploaded = self.published_at.as_deref().and_then(parse_iso8601_loose);
		let title = self.title.filter(|s| !s.trim().is_empty());
		let url = self
			.url
			.clone()
			.map(|u| absolutize(&u))
			.unwrap_or_else(|| format!("{SITE_URL}/chapter/{}", self.id));
		// Locked: paid chapter the user hasn't bought. Free donate/sub chapters
		// the user already paid for surface as is_user_paid=true.
		let locked = (self.is_donate || self.is_subscription) && !self.is_user_paid;
		Chapter {
			key: self.id.to_string(),
			title,
			chapter_number,
			volume_number,
			date_uploaded,
			url: Some(url),
			locked,
			..Default::default()
		}
	}
}

pub fn merge_book_into(book: BookFull, mut current: Manga) -> Manga {
	current.key = book.slug.clone();
	current.title = book.title;
	if let Some(cover) = book.vertical_image.as_ref().and_then(|i| i.url.clone()) {
		current.cover = Some(absolutize(&cover));
	}
	current.url = Some(format!("{SITE_URL}/{}", book.slug));

	// Description sometimes empty; combine with additionalInfo when present
	// for richer context.
	let pieces: Vec<String> = [book.description, book.additional_info]
		.into_iter()
		.flatten()
		.map(|s| strip_html(&s))
		.filter(|s| !s.is_empty())
		.collect();
	if !pieces.is_empty() {
		current.description = Some(pieces.join("\n\n"));
	}

	current.status = parse_status(book.status.as_deref());

	if let Some(a) = book.author.filter(|s| !s.is_empty()) {
		current.authors = Some(alloc::vec![a]);
	}

	let tags: Vec<String> = book
		.genres
		.unwrap_or_default()
		.into_iter()
		.filter_map(|g| g.label())
		.collect();
	if !tags.is_empty() {
		current.tags = Some(tags);
	}

	current.viewer = Viewer::Vertical;
	current.content_rating = ContentRating::Safe;
	current
}

// --- chapter page ---

#[derive(Deserialize)]
pub struct ChapterPageProps {
	#[serde(default)]
	pub chapter: Option<ChapterPageDto>,
}

#[derive(Deserialize)]
pub struct ChapterPageDto {
	#[serde(default)]
	pub content: Option<ChapterContent>,
	#[serde(default)]
	pub text: Option<String>,
}

#[derive(Deserialize)]
pub struct ChapterContent {
	#[serde(default)]
	pub text: Option<String>,
}

impl ChapterPageDto {
	pub fn extract_text(self) -> Option<String> {
		self.content
			.and_then(|c| c.text)
			.or(self.text)
	}
}

// --- helpers ---

pub fn strip_html(html: &str) -> String {
	let mut out = String::with_capacity(html.len());
	let mut in_tag = false;
	let mut tag_buf = String::new();
	let mut skip_until_close: Option<String> = None;
	let bytes = html.as_bytes();
	let mut i = 0;
	while i < bytes.len() {
		let c = match core::str::from_utf8(&bytes[i..]) {
			Ok(_) => bytes[i] as char,
			Err(_) => {
				i += 1;
				continue;
			}
		};
		if let Some(close) = skip_until_close.as_ref() {
			if c == '<' {
				let candidate = format!("</{}>", close);
				if html[i..].to_lowercase().starts_with(&candidate) {
					i += candidate.len();
					skip_until_close = None;
					continue;
				}
			}
			i += 1;
			continue;
		}
		if in_tag {
			if c == '>' {
				in_tag = false;
				let raw = tag_buf.clone();
				let lower = raw.to_lowercase();
				let lower = lower.trim();
				let tag_name: String = lower
					.chars()
					.take_while(|c| c.is_ascii_alphanumeric())
					.collect();
				if tag_name == "p" || tag_name == "br" || tag_name == "div" {
					if !out.ends_with("\n\n") {
						if !out.ends_with('\n') {
							out.push('\n');
						}
						out.push('\n');
					}
				} else if tag_name == "script" || tag_name == "style" {
					if !lower.starts_with('/') {
						skip_until_close = Some(tag_name);
					}
				} else if lower.starts_with("div") && lower.contains("messageblock") {
					skip_until_close = Some("div".to_string());
				}
				tag_buf.clear();
				i += 1;
				continue;
			}
			tag_buf.push(c);
			i += 1;
			continue;
		}
		if c == '<' {
			in_tag = true;
			tag_buf.clear();
			i += 1;
			continue;
		}
		out.push(c);
		i += 1;
	}
	let out = decode_entities(&out);
	collapse_newlines(&out).trim().to_string()
}

fn decode_entities(s: &str) -> String {
	s.replace("&nbsp;", " ")
		.replace("&amp;", "&")
		.replace("&lt;", "<")
		.replace("&gt;", ">")
		.replace("&quot;", "\"")
		.replace("&#039;", "'")
}

fn collapse_newlines(s: &str) -> String {
	let mut out = String::with_capacity(s.len());
	let mut run = 0usize;
	for c in s.chars() {
		if c == '\n' {
			run += 1;
			if run <= 2 {
				out.push(c);
			}
		} else {
			run = 0;
			out.push(c);
		}
	}
	out
}

// "2023-10-04 17:00:00" -> unix seconds.
fn parse_iso8601_loose(s: &str) -> Option<i64> {
	let s = s.trim();
	if s.len() < 19 {
		return None;
	}
	let year: i64 = s.get(0..4)?.parse().ok()?;
	let month: i64 = s.get(5..7)?.parse().ok()?;
	let day: i64 = s.get(8..10)?.parse().ok()?;
	let hour: i64 = s.get(11..13)?.parse().ok()?;
	let minute: i64 = s.get(14..16)?.parse().ok()?;
	let second: i64 = s.get(17..19)?.parse().ok()?;
	let days = days_from_civil(year, month as u32, day as u32);
	Some(days * 86400 + hour * 3600 + minute * 60 + second)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
	let y = if m <= 2 { y - 1 } else { y };
	let era = if y >= 0 { y } else { y - 399 } / 400;
	let yoe = (y - era * 400) as u64;
	let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) as u64 + 2) / 5 + d as u64 - 1;
	let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
	era * 146097 + doe as i64 - 719468
}
