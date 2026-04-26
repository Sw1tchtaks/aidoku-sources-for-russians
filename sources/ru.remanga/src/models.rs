use aidoku::{Chapter, ContentRating, Manga, MangaStatus, Viewer};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::Deserialize;

const SITE_URL: &str = "https://remanga.org";
const API_URL: &str = "https://api.remanga.org";

#[derive(Deserialize)]
pub struct Page<T> {
	pub results: Vec<T>,
	#[serde(default)]
	pub next: Option<String>,
}

#[derive(Deserialize)]
pub struct Envelope<T> {
	pub content: T,
}

#[derive(Deserialize)]
pub struct CatalogItem {
	pub dir: String,
	pub main_name: Option<String>,
	#[serde(default)]
	pub secondary_name: Option<String>,
	#[serde(default)]
	pub cover: Option<Cover>,
	#[serde(default, rename = "type")]
	pub kind: Option<NameId>,
	#[serde(default)]
	pub status: Option<NameId>,
	#[serde(default)]
	pub is_forbidden: bool,
}

#[derive(Deserialize)]
pub struct Cover {
	#[serde(default)]
	pub high: Option<String>,
	#[serde(default)]
	pub mid: Option<String>,
	#[serde(default)]
	pub low: Option<String>,
}

#[derive(Deserialize)]
pub struct NameId {
	#[serde(default)]
	pub id: i64,
	#[serde(default)]
	pub name: Option<String>,
}

impl CatalogItem {
	pub fn into_manga(self) -> Manga {
		let title = self
			.main_name
			.or(self.secondary_name)
			.unwrap_or_else(|| self.dir.clone());
		let cover = self.cover.and_then(|c| c.high.or(c.mid).or(c.low)).map(absolutize);
		let mut content_rating = ContentRating::Safe;
		if self.is_forbidden {
			content_rating = ContentRating::NSFW;
		}
		Manga {
			key: self.dir.clone(),
			title,
			cover,
			url: Some(format!("{SITE_URL}/manga/{}", self.dir)),
			status: parse_status(self.status.as_ref()),
			viewer: parse_viewer(self.kind.as_ref()),
			content_rating,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct DetailsItem {
	pub id: i64,
	pub dir: String,
	pub main_name: Option<String>,
	#[serde(default)]
	pub secondary_name: Option<String>,
	#[serde(default)]
	pub another_name: Option<String>,
	#[serde(default)]
	pub description: Option<String>,
	#[serde(default)]
	pub cover: Option<Cover>,
	#[serde(default, rename = "type")]
	pub kind: Option<NameId>,
	#[serde(default)]
	pub status: Option<NameId>,
	#[serde(default)]
	pub age_limit: Option<NameId>,
	#[serde(default)]
	pub branches: Vec<Branch>,
	#[serde(default)]
	pub genres: Vec<NameId>,
	#[serde(default)]
	pub categories: Vec<NameId>,
	#[serde(default)]
	pub publishers: Vec<NameId>,
	#[serde(default)]
	pub is_yaoi: bool,
	#[serde(default)]
	pub is_erotic: bool,
}

#[derive(Deserialize)]
pub struct Branch {
	pub id: i64,
	#[serde(default)]
	pub count_chapters: i32,
}

impl DetailsItem {
	pub fn primary_branch_id(&self) -> Option<i64> {
		self.branches
			.iter()
			.max_by_key(|b| b.count_chapters)
			.map(|b| b.id)
	}

	pub fn merge_into(self, mut current: Manga) -> Manga {
		current.key = self.dir.clone();
		current.title = self
			.main_name
			.unwrap_or_else(|| self.dir.clone());
		current.cover = self.cover.and_then(|c| c.high.or(c.mid).or(c.low)).map(absolutize);
		current.url = Some(format!("{SITE_URL}/manga/{}", self.dir));
		current.description = self.description.map(strip_html);

		let mut tags: Vec<String> = Vec::new();
		for c in &self.categories {
			if let Some(n) = &c.name {
				tags.push(n.clone());
			}
		}
		for g in &self.genres {
			if let Some(n) = &g.name {
				tags.push(n.clone());
			}
		}
		if let Some(age) = self.age_limit.as_ref().and_then(|a| a.name.clone()) {
			tags.push(age);
		}
		if !tags.is_empty() {
			current.tags = Some(tags);
		}

		let publishers: Vec<String> = self
			.publishers
			.iter()
			.filter_map(|p| p.name.clone())
			.collect();
		if !publishers.is_empty() {
			current.authors = Some(publishers);
		}

		current.status = parse_status(self.status.as_ref());
		current.viewer = parse_viewer(self.kind.as_ref());

		// content rating
		if self.is_erotic
			|| self.is_yaoi
			|| self
				.age_limit
				.as_ref()
				.and_then(|a| a.name.clone())
				.map(|n| n.contains("18"))
				.unwrap_or(false)
		{
			current.content_rating = ContentRating::NSFW;
		} else if self
			.age_limit
			.as_ref()
			.and_then(|a| a.name.clone())
			.map(|n| n.contains("16"))
			.unwrap_or(false)
		{
			current.content_rating = ContentRating::Suggestive;
		} else {
			current.content_rating = ContentRating::Safe;
		}

		current
	}
}

#[derive(Deserialize)]
pub struct ChapterDto {
	pub id: i64,
	#[serde(default)]
	pub chapter: Option<String>,
	#[serde(default)]
	pub tome: Option<i32>,
	#[serde(default)]
	pub name: Option<String>,
	#[serde(default)]
	pub upload_date: Option<String>,
	#[serde(default)]
	pub is_paid: bool,
	#[serde(default)]
	pub publishers: Vec<NameId>,
}

impl ChapterDto {
	pub fn into_chapter(self, manga_dir: &str) -> Chapter {
		let chapter_number = self.chapter.as_deref().and_then(|s| s.parse::<f32>().ok());
		let volume_number = self.tome.map(|v| v as f32);
		let date_uploaded = self.upload_date.as_deref().and_then(parse_iso8601);
		let scanlators: Vec<String> = self
			.publishers
			.iter()
			.filter_map(|p| p.name.clone())
			.collect();
		let title = self.name.filter(|s| !s.is_empty());
		Chapter {
			key: self.id.to_string(),
			title,
			chapter_number,
			volume_number,
			date_uploaded,
			scanlators: if scanlators.is_empty() {
				None
			} else {
				Some(scanlators)
			},
			url: Some(format!("{SITE_URL}/manga/{}/ch{}", manga_dir, self.id)),
			locked: self.is_paid,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct ChapterPagesContent {
	#[serde(default)]
	pub pages: serde_json::Value,
}

// Pages can come as:
//   pages: [ {id, link, ...}, ... ]            // single layout
//   pages: [ [ {id, link}, {id, link} ], ... ] // double-page layout
pub fn flatten_pages(value: &serde_json::Value) -> Vec<String> {
	let mut out = Vec::new();
	if let Some(arr) = value.as_array() {
		for item in arr {
			if let Some(inner) = item.as_array() {
				for p in inner {
					if let Some(url) = page_url(p) {
						out.push(url);
					}
				}
			} else if let Some(url) = page_url(item) {
				out.push(url);
			}
		}
	}
	out
}

fn page_url(v: &serde_json::Value) -> Option<String> {
	v.get("link").and_then(|x| x.as_str()).map(|s| s.to_string())
}

fn parse_status(s: Option<&NameId>) -> MangaStatus {
	match s.map(|x| x.id).unwrap_or(-1) {
		1 => MangaStatus::Completed,
		2 => MangaStatus::Ongoing,
		3 => MangaStatus::Hiatus,
		4 => MangaStatus::Cancelled,
		5 => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}

fn parse_viewer(t: Option<&NameId>) -> Viewer {
	match t.map(|x| x.id).unwrap_or(0) {
		// 1=manga, 2=manhwa, 3=manhua, 4=Western, 5=RuManga, 6=Single, 7=OEL
		2 => Viewer::Webtoon,
		3 => Viewer::Webtoon,
		4 => Viewer::LeftToRight,
		7 => Viewer::LeftToRight,
		_ => Viewer::RightToLeft,
	}
}

fn absolutize(path: String) -> String {
	if path.starts_with("http://") || path.starts_with("https://") {
		path
	} else if let Some(p) = path.strip_prefix("//") {
		format!("https://{p}")
	} else if path.starts_with('/') {
		format!("{API_URL}{path}")
	} else {
		format!("{API_URL}/{path}")
	}
}

fn strip_html(s: String) -> String {
	let mut out = String::with_capacity(s.len());
	let mut inside_tag = false;
	for c in s.chars() {
		match c {
			'<' => inside_tag = true,
			'>' => inside_tag = false,
			_ if !inside_tag => out.push(c),
			_ => {}
		}
	}
	out
}

// Parse "2024-08-12T10:23:45.123456Z" or "2024-08-12T10:23:45" -> unix seconds.
fn parse_iso8601(s: &str) -> Option<i64> {
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
