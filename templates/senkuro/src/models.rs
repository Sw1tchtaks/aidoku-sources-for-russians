use crate::settings::prefer_english_titles;
use aidoku::{Chapter, ContentRating, Manga, MangaStatus, Viewer};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::Deserialize;

// --- filters (allLabels) ---

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiltersResponse {
	#[serde(default)]
	pub all_labels: Vec<LabelDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelDto {
	#[serde(default)]
	pub id: Option<String>,
	#[serde(default)]
	pub root_id: Option<String>,
	#[serde(default)]
	pub depth: Option<i32>,
	#[serde(default)]
	pub rating: Option<String>,
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
	#[serde(default)]
	pub preview: Option<ImageSize>,
}

// --- catalog/search ---

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MangasData {
	pub manga_tachiyomi_search: Option<MangasResult>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PopularByPeriodData {
	#[serde(default)]
	pub manga_popular_by_period: Vec<SearchManga>,
}

#[derive(Deserialize, Default)]
pub struct MangaConnectionData {
	#[serde(default)]
	pub mangas: Option<MangaConnection>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MangaConnection {
	#[serde(default)]
	pub edges: Vec<MangaEdge>,
	#[serde(default)]
	pub page_info: PageInfo,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
	#[serde(default)]
	pub has_next_page: bool,
	#[serde(default)]
	pub end_cursor: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct MangaEdge {
	pub node: SearchManga,
}

#[derive(Deserialize, Default)]
pub struct MangasResult {
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
	#[serde(rename = "type")]
	pub kind: Option<String>,
	#[serde(default)]
	pub rating: Option<String>,
	#[serde(default)]
	pub formats: Option<Vec<String>>,
	#[serde(default)]
	pub score: Option<f32>,
	#[serde(default)]
	pub cover: Option<Image>,
	#[serde(default)]
	pub last_chapters: Vec<ChapterDto>,
}

impl SearchManga {
	pub fn into_manga(self, base_url: &str) -> Manga {
		let title = pick_title(
			&self.titles,
			self.original_name.as_ref().map(|t| t.content.as_str()),
			&self.slug,
		);
		let cover = self
			.cover
			.and_then(|c| c.preview.or(c.original).map(|x| x.url));
		let mut tags = Vec::new();
		if let Some(kind) = self.kind.as_deref().and_then(display_manga_type) {
			tags.push(kind.to_string());
		}
		if let Some(formats) = &self.formats {
			for f in formats.iter().filter_map(|f| display_manga_format(f)) {
				if !tags.iter().any(|t| t.as_str() == f) {
					tags.push(f.to_string());
				}
			}
		}
		let description = update_description(&self.last_chapters).or_else(|| {
			self.score
				.filter(|score| *score > 0.0)
				.map(|score| alloc::format!("★ {score:.1}"))
		});
		Manga {
			key: build_manga_key(&self.id, &self.slug),
			title,
			cover,
			url: Some(alloc::format!("{}/manga/{}", base_url, self.slug)),
			description,
			tags: if tags.is_empty() { None } else { Some(tags) },
			content_rating: parse_rating(self.rating.as_deref()),
			viewer: parse_viewer(self.kind.as_deref()),
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
	pub score: Option<f32>,
	#[serde(default)]
	pub formats: Option<Vec<String>>,
	#[serde(default)]
	pub translation_status: Option<String>,
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
		let cover = self
			.cover
			.and_then(|c| c.preview.or(c.original).map(|x| x.url));

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

		// Keep the type, localized labels, and localized formats in a stable order.
		let mut tags: Vec<String> = Vec::new();
		if let Some(kind) = self.kind.as_deref().and_then(display_manga_type) {
			push_unique(&mut tags, kind.to_string());
		}
		for label in &self.labels {
			if let Some(title) = pick_label_title(&label.titles) {
				push_unique(&mut tags, title);
			}
		}
		if let Some(formats) = &self.formats {
			for format in formats.iter().filter_map(|format| display_manga_format(format)) {
				push_unique(&mut tags, format.to_string());
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
		let mut description_blocks: Vec<String> = Vec::new();
		if let Some(alt_names) = alt_names {
			description_blocks.push(alt_names);
		}
		let mut metadata: Vec<String> = Vec::new();
		if let Some(score) = self.score.filter(|score| *score > 0.0) {
			metadata.push(alloc::format!("Рейтинг: {score:.1}"));
		}
		if let Some(status) = self
			.translation_status
			.as_deref()
			.and_then(display_translation_status)
		{
			metadata.push(alloc::format!("Перевод: {status}"));
		}
		if !metadata.is_empty() {
			description_blocks.push(metadata.join(" • "));
		}
		if let Some(description) = description {
			description_blocks.push(description);
		}
		let description = if description_blocks.is_empty() {
			None
		} else {
			Some(description_blocks.join("\n\n"))
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

fn update_description(chapters: &[ChapterDto]) -> Option<String> {
	let chapter = chapters.first()?;
	let mut out = chapter
		.number
		.as_ref()
		.map(|number| alloc::format!("Глава {number}"))
		.unwrap_or_else(|| "Новая глава".to_string());
	if let Some(name) = chapter.name.as_ref().filter(|s| !s.is_empty()) {
		out.push_str(" - ");
		out.push_str(name);
	}
	if let Some(date) = chapter.created_at.as_deref().and_then(format_iso_date) {
		out.push_str(" • ");
		out.push_str(&date);
	}
	Some(out)
}

fn format_iso_date(s: &str) -> Option<String> {
	if s.len() < 10 {
		return None;
	}
	let year = s.get(0..4)?;
	let month = s.get(5..7)?;
	let day = s.get(8..10)?;
	Some(alloc::format!("{day}.{month}.{year}"))
}

fn display_manga_type(value: &str) -> Option<&'static str> {
	match value {
		"MANGA" => Some("Манга"),
		"MANHWA" => Some("Манхва"),
		"MANHUA" => Some("Маньхуа"),
		"COMICS" => Some("Комикс"),
		"OEL_MANGA" => Some("OEL-манга"),
		"RU_MANGA" => Some("РуМанга"),
		_ => None,
	}
}

fn display_manga_format(value: &str) -> Option<&'static str> {
	match value {
		"IN_COLOR" => Some("В цвете"),
		"WEB" => Some("WEB"),
		"WEBTOON" => Some("WEBTOON"),
		"DIGEST" => Some("Сборник"),
		"DOUJINSHI" => Some("Додзинси"),
		"SINGLE" => Some("Сингл"),
		"YONKOMA" => Some("Ёнкома"),
		"SHORT" => Some("Короткое"),
		_ => None,
	}
}

fn display_translation_status(value: &str) -> Option<&'static str> {
	match value {
		"IN_PROGRESS" => Some("переводится"),
		"FINISHED" => Some("завершён"),
		"FROZEN" => Some("заморожен"),
		"ABANDONED" => Some("заброшен"),
		_ => None,
	}
}

fn push_unique(items: &mut Vec<String>, value: String) {
	if !items.iter().any(|item| item == &value) {
		items.push(value);
	}
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
