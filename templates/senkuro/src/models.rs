use crate::settings::prefer_english_titles;
use aidoku::{Chapter, ContentRating, Manga, MangaStatus, Viewer};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::Deserialize;

// --- filters (mangaTachiyomiSearchFilters) ---

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiltersResponse {
	pub manga_tachiyomi_search_filters: Option<FiltersPayload>,
}

#[derive(Deserialize, Default)]
pub struct FiltersPayload {
	#[serde(default)]
	pub labels: Vec<LabelDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelDto {
	#[serde(default)]
	pub id: Option<String>,
	#[serde(default)]
	pub root_id: Option<String>,
	pub slug: String,
	#[serde(default)]
	pub titles: Vec<I18nString>,
}

impl LabelDto {
	pub fn display_name(&self) -> String {
		pick_label_title(&self.titles).unwrap_or_else(|| self.slug.clone())
	}
}

#[derive(Deserialize)]
pub struct GqlResponse<T> {
	pub data: Option<T>,
	#[serde(default)]
	pub errors: Option<Vec<GqlError>>,
}

#[derive(Deserialize)]
pub struct GqlError {
	pub message: String,
}

#[derive(Deserialize)]
pub struct I18nString {
	pub lang: String,
	pub content: String,
}

#[derive(Deserialize)]
pub struct ImageSize {
	pub url: String,
}

#[derive(Deserialize, Default)]
pub struct Image {
	#[serde(default)]
	pub original: Option<ImageSize>,
}

// --- search ---

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchData {
	pub manga_tachiyomi_search: Option<SearchPayload>,
}

#[derive(Deserialize, Default)]
pub struct SearchPayload {
	#[serde(default)]
	pub mangas: Vec<SearchManga>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchManga {
	pub id: String,
	pub slug: String,
	#[serde(default)]
	pub original_name: Option<I18nString>,
	#[serde(default)]
	pub titles: Vec<I18nString>,
	#[serde(default)]
	pub cover: Option<Image>,
}

impl SearchManga {
	pub fn into_manga(self, base_url: &str) -> Manga {
		let title = pick_title(
			&self.titles,
			self.original_name.as_ref().map(|t| t.content.as_str()),
			&self.slug,
		);
		let cover = self.cover.and_then(|c| c.original.map(|x| x.url));
		Manga {
			key: build_manga_key(&self.id, &self.slug),
			title,
			cover,
			url: Some(alloc::format!("{}/manga/{}", base_url, self.slug)),
			..Default::default()
		}
	}
}

// --- details ---

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailsData {
	pub manga_tachiyomi_info: Option<MangaInfo>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaInfo {
	pub id: String,
	pub slug: String,
	#[serde(default)]
	pub original_name: Option<I18nString>,
	#[serde(default)]
	pub titles: Vec<I18nString>,
	#[serde(default)]
	pub alternative_names: Vec<I18nString>,
	#[serde(default)]
	pub localizations: Vec<Localization>,
	#[serde(default, rename = "type")]
	pub kind: Option<String>,
	#[serde(default)]
	pub status: Option<String>,
	#[serde(default)]
	pub rating: Option<String>,
	#[serde(default)]
	pub formats: Option<Vec<String>>,
	#[serde(default)]
	pub labels: Vec<Label>,
	#[serde(default)]
	pub cover: Option<Image>,
	#[serde(default)]
	pub main_staff: Option<Vec<Staff>>,
}

#[derive(Deserialize)]
pub struct Localization {
	pub lang: String,
	#[serde(default)]
	pub description: Option<String>,
}

#[derive(Deserialize)]
pub struct Label {
	#[serde(default)]
	pub titles: Vec<I18nString>,
}

#[derive(Deserialize)]
pub struct Staff {
	#[serde(default)]
	pub roles: Vec<String>,
	pub person: Person,
}

#[derive(Deserialize)]
pub struct Person {
	pub name: String,
}

impl MangaInfo {
	pub fn into_manga(self, base_url: &str) -> Manga {
		let title = pick_title(
			&self.titles,
			self.original_name.as_ref().map(|t| t.content.as_str()),
			&self.slug,
		);
		let cover = self.cover.and_then(|c| c.original.map(|x| x.url));

		let mut authors = Vec::new();
		let mut artists = Vec::new();
		if let Some(staff) = &self.main_staff {
			for s in staff {
				if s.roles.iter().any(|r| r == "STORY") {
					authors.push(s.person.name.clone());
				}
				if s.roles.iter().any(|r| r == "ART") {
					artists.push(s.person.name.clone());
				}
			}
		}

		// Description: prefer RU, fall back to EN
		let prefer_en = prefer_english_titles();
		let primary = if prefer_en { "EN" } else { "RU" };
		let secondary = if prefer_en { "RU" } else { "EN" };
		let description = self
			.localizations
			.iter()
			.find(|l| l.lang == primary)
			.and_then(|l| l.description.clone())
			.or_else(|| {
				self.localizations
					.iter()
					.find(|l| l.lang == secondary)
					.and_then(|l| l.description.clone())
			});

		// Tags: localized label names
		let mut tags: Vec<String> = self
			.labels
			.iter()
			.filter_map(|l| pick_label_title(&l.titles))
			.collect();
		// Append formats as pseudo-tags for visibility
		if let Some(formats) = &self.formats {
			for f in formats {
				tags.push(f.clone());
			}
		}

		// Pre-compose alt-names block at the top of description
		let alt_names = if !self.alternative_names.is_empty() {
			let joined = self
				.alternative_names
				.iter()
				.map(|n| n.content.clone())
				.collect::<Vec<_>>()
				.join(" / ");
			Some(alloc::format!("Альтернативные названия:\n{joined}"))
		} else {
			None
		};
		let description = match (alt_names, description) {
			(Some(a), Some(d)) => Some(alloc::format!("{a}\n\n{d}")),
			(Some(a), None) => Some(a),
			(None, d) => d,
		};

		Manga {
			key: build_manga_key(&self.id, &self.slug),
			title,
			cover,
			authors: if authors.is_empty() {
				None
			} else {
				Some(authors)
			},
			artists: if artists.is_empty() {
				None
			} else {
				Some(artists)
			},
			description,
			url: Some(alloc::format!("{}/manga/{}", base_url, self.slug)),
			tags: if tags.is_empty() { None } else { Some(tags) },
			status: parse_status(self.status.as_deref()),
			content_rating: parse_rating(self.rating.as_deref()),
			viewer: parse_viewer(self.kind.as_deref()),
			..Default::default()
		}
	}
}

// --- chapters ---

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChaptersData {
	pub manga_tachiyomi_chapters: Option<ChaptersPayload>,
}

#[derive(Deserialize, Default)]
pub struct ChaptersPayload {
	#[serde(default)]
	pub chapters: Vec<ChapterDto>,
	#[serde(default)]
	pub teams: Vec<TeamDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterDto {
	pub id: String,
	pub slug: String,
	#[serde(default)]
	pub name: Option<String>,
	#[serde(default)]
	pub team_ids: Vec<String>,
	#[serde(default)]
	pub number: Option<String>,
	#[serde(default)]
	pub volume: Option<String>,
	#[serde(default)]
	pub created_at: Option<String>,
}

#[derive(Deserialize)]
pub struct TeamDto {
	pub id: String,
	pub name: String,
}

impl ChapterDto {
	pub fn into_chapter(self, base_url: &str, manga_slug: &str, teams: &[TeamDto]) -> Chapter {
		let chapter_number = self.number.as_deref().and_then(|s| s.parse::<f32>().ok());
		let volume_number = self.volume.as_deref().and_then(|s| s.parse::<f32>().ok());
		let date_uploaded = self.created_at.as_deref().and_then(parse_iso8601);
		let scanlators: Vec<String> = teams
			.iter()
			.filter(|t| self.team_ids.iter().any(|id| id == &t.id))
			.map(|t| t.name.clone())
			.collect();
		let title = self.name.filter(|s| !s.is_empty());
		Chapter {
			key: alloc::format!("{},,{}", self.id, self.slug),
			title,
			chapter_number,
			volume_number,
			date_uploaded,
			scanlators: if scanlators.is_empty() {
				None
			} else {
				Some(scanlators)
			},
			url: Some(alloc::format!(
				"{}/manga/{}/chapter/{}",
				base_url,
				manga_slug,
				self.slug
			)),
			..Default::default()
		}
	}
}

// --- pages ---

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PagesData {
	pub manga_tachiyomi_chapter_pages: Option<PagesPayload>,
}

#[derive(Deserialize, Default)]
pub struct PagesPayload {
	#[serde(default)]
	pub pages: Vec<PageDto>,
}

#[derive(Deserialize)]
pub struct PageDto {
	pub url: String,
}

// --- helpers ---

pub fn build_manga_key(id: &str, slug: &str) -> String {
	alloc::format!("{},,{}", id, slug)
}

pub fn split_manga_key(key: &str) -> (&str, &str) {
	match key.split_once(",,") {
		Some((id, slug)) => (id, slug),
		None => (key, key),
	}
}

pub fn split_chapter_key(key: &str) -> (&str, &str) {
	match key.split_once(",,") {
		Some((id, slug)) => (id, slug),
		None => (key, key),
	}
}

fn pick_title(titles: &[I18nString], original: Option<&str>, fallback: &str) -> String {
	let prefer_en = prefer_english_titles();
	let primary = if prefer_en { "EN" } else { "RU" };
	let secondary = if prefer_en { "RU" } else { "EN" };
	if let Some(t) = titles.iter().find(|t| t.lang == primary) {
		return t.content.clone();
	}
	if let Some(t) = titles.iter().find(|t| t.lang == secondary) {
		return t.content.clone();
	}
	if let Some(t) = titles.first() {
		return t.content.clone();
	}
	original
		.map(|s| s.to_string())
		.unwrap_or_else(|| fallback.to_string())
}

pub fn pick_label_title(titles: &[I18nString]) -> Option<String> {
	let prefer_en = prefer_english_titles();
	let primary = if prefer_en { "EN" } else { "RU" };
	titles
		.iter()
		.find(|t| t.lang == primary)
		.or_else(|| titles.first())
		.map(|t| t.content.clone())
}

fn parse_status(status: Option<&str>) -> MangaStatus {
	match status {
		Some("ONGOING") | Some("ANNOUNCE") => MangaStatus::Ongoing,
		Some("FINISHED") | Some("COMPLETED") => MangaStatus::Completed,
		Some("HIATUS") | Some("PAUSED") => MangaStatus::Hiatus,
		Some("CANCELLED") | Some("DROPPED") => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}

fn parse_rating(rating: Option<&str>) -> ContentRating {
	match rating {
		Some("EXPLICIT") | Some("ADULT") | Some("PORNOGRAPHIC") => ContentRating::NSFW,
		Some("QUESTIONABLE") | Some("SENSITIVE") | Some("SUGGESTIVE") => ContentRating::Suggestive,
		_ => ContentRating::Safe,
	}
}

fn parse_viewer(kind: Option<&str>) -> Viewer {
	match kind {
		Some("MANHWA") | Some("MANHUA") | Some("WEBTOON") => Viewer::Webtoon,
		Some("MANGA") | Some("RU_MANGA") => Viewer::RightToLeft,
		Some("COMICS") | Some("OEL_MANGA") => Viewer::LeftToRight,
		_ => Viewer::RightToLeft,
	}
}

// Parse ISO 8601 like "2025-08-03T15:12:50.594621" -> unix seconds (UTC).
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

// Howard Hinnant's days_from_civil.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
	let y = if m <= 2 { y - 1 } else { y };
	let era = if y >= 0 { y } else { y - 399 } / 400;
	let yoe = (y - era * 400) as u64;
	let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) as u64 + 2) / 5 + d as u64 - 1;
	let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
	era * 146097 + doe as i64 - 719468
}
