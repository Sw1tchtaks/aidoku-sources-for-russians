use crate::settings::{prefer_english_titles, prefer_original_quality};
use aidoku::{Chapter, ContentRating, Manga, MangaStatus, Viewer};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::Deserialize;

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
pub struct I18nTitle {
	pub lang: String,
	pub content: String,
}

#[derive(Deserialize)]
pub struct ImageSize {
	pub url: String,
}

#[derive(Deserialize)]
pub struct Image {
	#[serde(default)]
	pub original: Option<ImageSize>,
	#[serde(default)]
	pub preview: Option<ImageSize>,
	#[serde(default)]
	pub compress: Option<ImageSize>,
}

// --- search response ---

#[derive(Deserialize)]
pub struct SearchData {
	pub search: Option<SearchConnection>,
}

#[derive(Deserialize)]
pub struct SearchConnection {
	#[serde(default)]
	pub edges: Vec<SearchEdge>,
}

#[derive(Deserialize)]
pub struct SearchEdge {
	pub node: Option<SearchNode>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchNode {
	pub slug: String,
	#[serde(default)]
	pub original_name: Option<String>,
	#[serde(default)]
	pub titles: Vec<I18nTitle>,
	#[serde(default)]
	pub manga_status: Option<String>,
	#[serde(default)]
	pub manga_rating: Option<String>,
	#[serde(default)]
	pub cover: Option<Image>,
}

impl SearchNode {
	pub fn into_manga(self) -> Manga {
		let title = pick_title(&self.titles, self.original_name.as_deref(), &self.slug);
		let cover = self.cover.and_then(pick_cover);
		Manga {
			key: self.slug,
			title,
			cover,
			..Default::default()
		}
	}
}

// --- fetchManga response ---

#[derive(Deserialize)]
pub struct MangaData {
	pub manga: Option<MangaNode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaNode {
	pub slug: String,
	#[serde(default)]
	pub original_name: Option<I18nTitleOrString>,
	#[serde(default)]
	pub titles: Vec<I18nTitle>,
	#[serde(default)]
	pub alternative_names: Vec<I18nTitle>,
	#[serde(default)]
	pub localizations: Vec<MangaLocalization>,
	#[serde(default, rename = "type")]
	pub kind: Option<String>,
	#[serde(default)]
	pub status: Option<String>,
	#[serde(default)]
	pub rating: Option<String>,
	#[serde(default)]
	pub cover: Option<Image>,
	#[serde(default)]
	pub labels: Vec<Label>,
	#[serde(default)]
	pub branches: Vec<Branch>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum I18nTitleOrString {
	Title(I18nTitle),
	String(String),
}

#[derive(Deserialize)]
pub struct MangaLocalization {
	pub lang: String,
	#[serde(default)]
	pub description: Option<DescriptionDoc>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum DescriptionDoc {
	String(String),
	Nodes(Vec<TiptapNode>),
}

#[derive(Deserialize)]
pub struct TiptapNode {
	#[serde(default)]
	pub text: Option<String>,
	#[serde(default)]
	pub content: Option<Vec<TiptapNode>>,
}

impl TiptapNode {
	pub fn flatten(&self, out: &mut String) {
		if let Some(t) = &self.text {
			out.push_str(t);
		}
		if let Some(children) = &self.content {
			for c in children {
				c.flatten(out);
			}
		}
	}
}

#[derive(Deserialize)]
pub struct Label {
	#[serde(default)]
	pub titles: Vec<I18nTitle>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
	pub id: String,
	#[serde(default)]
	pub lang: Option<String>,
	#[serde(default)]
	pub primary_branch: bool,
	#[serde(default)]
	pub chapters: Option<i32>,
}

impl MangaNode {
	pub fn into_manga(self, base_url: &str, needs_details: bool) -> Manga {
		let mut item = Manga {
			key: self.slug.clone(),
			..Default::default()
		};

		// title
		let original = match &self.original_name {
			Some(I18nTitleOrString::Title(t)) => Some(t.content.as_str()),
			Some(I18nTitleOrString::String(s)) => Some(s.as_str()),
			None => None,
		};
		item.title = pick_title(&self.titles, original, &self.slug);

		// cover
		item.cover = self.cover.and_then(pick_cover);

		// url
		item.url = Some(alloc::format!("{}/manga/{}", base_url, self.slug));

		if !needs_details {
			return item;
		}

		// description: prefer Russian, fall back to English
		item.description = pick_description(&self.localizations);

		// tags from labels
		let mut tags: Vec<String> = Vec::new();
		for label in &self.labels {
			if let Some(name) = pick_label_title(&label.titles) {
				tags.push(name);
			}
		}
		if !tags.is_empty() {
			item.tags = Some(tags);
		}

		// status
		item.status = match self.status.as_deref() {
			Some("ONGOING") => MangaStatus::Ongoing,
			Some("COMPLETED") | Some("FINISHED") => MangaStatus::Completed,
			Some("HIATUS") | Some("PAUSED") => MangaStatus::Hiatus,
			Some("CANCELLED") | Some("DROPPED") => MangaStatus::Cancelled,
			_ => MangaStatus::Unknown,
		};

		// rating
		item.content_rating = match self.rating.as_deref() {
			Some("EXPLICIT") | Some("ADULT") | Some("PORNOGRAPHIC") => ContentRating::NSFW,
			Some("SENSITIVE") | Some("SUGGESTIVE") => ContentRating::Suggestive,
			_ => ContentRating::Safe,
		};

		// viewer
		item.viewer = match self.kind.as_deref() {
			Some("MANHWA") | Some("MANHUA") | Some("WEBTOON") => Viewer::Webtoon,
			Some("MANGA") => Viewer::RightToLeft,
			Some("COMICS") | Some("OEL") | Some("ORIGINAL") => Viewer::LeftToRight,
			_ => Viewer::RightToLeft,
		};

		item
	}

	pub fn primary_branch_id(&self) -> Option<String> {
		self.branches
			.iter()
			.find(|b| b.primary_branch && b.lang.as_deref() == Some("RU"))
			.or_else(|| self.branches.iter().find(|b| b.primary_branch))
			.or_else(|| self.branches.iter().find(|b| b.lang.as_deref() == Some("RU")))
			.or_else(|| self.branches.first())
			.map(|b| b.id.clone())
	}
}

// --- fetchMangaChapters response ---

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChaptersData {
	pub manga_chapters: Option<ChaptersConnection>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChaptersConnection {
	#[serde(default)]
	pub edges: Vec<ChapterEdge>,
	#[serde(default)]
	pub page_info: Option<PageInfo>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
	#[serde(default)]
	pub has_next_page: bool,
	#[serde(default)]
	pub end_cursor: Option<String>,
}

#[derive(Deserialize)]
pub struct ChapterEdge {
	pub node: Option<ChapterNode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterNode {
	pub slug: String,
	#[serde(default)]
	pub name: Option<String>,
	#[serde(default)]
	pub number: Option<String>,
	#[serde(default)]
	pub volume: Option<String>,
	#[serde(default)]
	pub created_at: Option<String>,
}

impl ChapterNode {
	pub fn into_chapter(self, base_url: &str, manga_slug: &str) -> Chapter {
		let chapter_number = self.number.as_deref().and_then(|s| s.parse::<f32>().ok());
		let volume_number = self.volume.as_deref().and_then(|s| s.parse::<f32>().ok());
		let date_uploaded = self.created_at.as_deref().and_then(parse_iso8601);
		Chapter {
			key: self.slug.clone(),
			title: self.name,
			chapter_number,
			volume_number,
			date_uploaded,
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

// --- fetchMangaChapter (pages) response ---

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterPagesData {
	pub manga_chapter: Option<ChapterPagesNode>,
}

#[derive(Deserialize)]
pub struct ChapterPagesNode {
	#[serde(default)]
	pub pages: Vec<ChapterPage>,
}

#[derive(Deserialize)]
pub struct ChapterPage {
	pub image: Option<Image>,
}

impl ChapterPage {
	pub fn into_url(self) -> Option<String> {
		let img = self.image?;
		let want_original = prefer_original_quality();
		if want_original {
			img.original
				.map(|x| x.url)
				.or_else(|| img.compress.map(|x| x.url))
		} else {
			img.compress
				.map(|x| x.url)
				.or_else(|| img.original.map(|x| x.url))
		}
	}
}

// --- helpers ---

fn pick_title(titles: &[I18nTitle], original: Option<&str>, fallback: &str) -> String {
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

fn pick_label_title(titles: &[I18nTitle]) -> Option<String> {
	let prefer_en = prefer_english_titles();
	let primary = if prefer_en { "EN" } else { "RU" };
	titles
		.iter()
		.find(|t| t.lang == primary)
		.or_else(|| titles.first())
		.map(|t| t.content.clone())
}

fn pick_cover(img: Image) -> Option<String> {
	img.original
		.map(|x| x.url)
		.or_else(|| img.preview.map(|x| x.url))
		.or_else(|| img.compress.map(|x| x.url))
}

fn pick_description(localizations: &[MangaLocalization]) -> Option<String> {
	let prefer_en = prefer_english_titles();
	let primary = if prefer_en { "EN" } else { "RU" };
	let secondary = if prefer_en { "RU" } else { "EN" };

	for lang in [primary, secondary] {
		if let Some(loc) = localizations.iter().find(|l| l.lang == lang) {
			if let Some(d) = render_description(&loc.description) {
				if !d.is_empty() {
					return Some(d);
				}
			}
		}
	}
	None
}

fn render_description(desc: &Option<DescriptionDoc>) -> Option<String> {
	let desc = desc.as_ref()?;
	match desc {
		DescriptionDoc::String(s) => Some(s.clone()),
		DescriptionDoc::Nodes(nodes) => {
			let mut out = String::new();
			for (i, n) in nodes.iter().enumerate() {
				if i > 0 {
					out.push_str("\n\n");
				}
				n.flatten(&mut out);
			}
			Some(out)
		}
	}
}

// Parse ISO 8601 timestamp like "2025-08-03T15:12:50.594621" → unix seconds (UTC).
fn parse_iso8601(s: &str) -> Option<i64> {
	let bytes = s.as_bytes();
	if bytes.len() < 19 {
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

// Howard Hinnant's days_from_civil algorithm.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
	let y = if m <= 2 { y - 1 } else { y };
	let era = if y >= 0 { y } else { y - 399 } / 400;
	let yoe = (y - era * 400) as u64;
	let doy = ((153 * (if m > 2 { m - 3 } else { m + 9 }) as u64 + 2) / 5 + d as u64 - 1) as u64;
	let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
	era * 146097 + doe as i64 - 719468
}
