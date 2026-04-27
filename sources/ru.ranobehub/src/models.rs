use aidoku::imports::defaults::defaults_get;
use aidoku::{Chapter, ContentRating, Manga, MangaStatus, Viewer};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::Deserialize;

const SITE_URL: &str = "https://ranobehub.org";

fn prefer_english() -> bool {
	defaults_get::<bool>("preferEnglishTitles").unwrap_or(false)
}

#[derive(Deserialize)]
pub struct Names {
	#[serde(default)]
	pub rus: Option<String>,
	#[serde(default)]
	pub eng: Option<String>,
}

impl Names {
	pub fn pick(&self, fallback: &str) -> String {
		let prefer_en = prefer_english();
		let primary = if prefer_en {
			self.eng.as_deref()
		} else {
			self.rus.as_deref()
		};
		let secondary = if prefer_en {
			self.rus.as_deref()
		} else {
			self.eng.as_deref()
		};
		primary
			.filter(|s| !s.is_empty())
			.or(secondary)
			.map(|s| s.to_string())
			.unwrap_or_else(|| fallback.to_string())
	}
}

#[derive(Deserialize)]
pub struct Poster {
	#[serde(default)]
	pub big: Option<String>,
	#[serde(default)]
	pub medium: Option<String>,
	#[serde(default)]
	pub small: Option<String>,
}

impl Poster {
	pub fn best_url(&self) -> Option<String> {
		self.medium
			.clone()
			.or_else(|| self.big.clone())
			.or_else(|| self.small.clone())
	}
}

#[derive(Deserialize)]
pub struct StatusDto {
	#[serde(default)]
	pub id: Option<i64>,
}

impl StatusDto {
	pub fn into_manga_status(self) -> MangaStatus {
		match self.id.unwrap_or(0) {
			1 => MangaStatus::Ongoing,
			2 => MangaStatus::Completed,
			3 => MangaStatus::Hiatus,
			4 => MangaStatus::Cancelled,
			_ => MangaStatus::Unknown,
		}
	}
}

// --- /api/search ---

#[derive(Deserialize)]
pub struct SearchEnvelope {
	pub resource: Vec<SearchItem>,
	#[serde(default)]
	pub pagination: Option<Pagination>,
}

#[derive(Deserialize)]
pub struct Pagination {
	#[serde(rename = "currentPage", default)]
	pub current_page: i32,
	#[serde(rename = "lastPage", default)]
	pub last_page: i32,
}

#[derive(Deserialize)]
pub struct SearchItem {
	pub id: i64,
	#[serde(default)]
	pub names: Option<Names>,
	#[serde(default)]
	pub url: Option<String>,
	#[serde(default)]
	pub poster: Option<Poster>,
	#[serde(default)]
	pub synopsis: Option<String>,
}

impl SearchItem {
	pub fn into_manga(self) -> Manga {
		let title = self
			.names
			.as_ref()
			.map(|n| n.pick(&self.id.to_string()))
			.unwrap_or_else(|| self.id.to_string());
		let cover = self.poster.as_ref().and_then(|p| p.best_url());
		let url = self
			.url
			.clone()
			.unwrap_or_else(|| format!("{SITE_URL}/ranobe/{}", self.id));
		Manga {
			key: self.id.to_string(),
			title,
			cover,
			url: Some(url),
			..Default::default()
		}
	}
}

// --- /api/ranobe/{id} ---

#[derive(Deserialize)]
pub struct DetailsEnvelope {
	pub data: DetailsItem,
}

#[derive(Deserialize)]
pub struct DetailsItem {
	pub id: i64,
	#[serde(default)]
	pub names: Option<Names>,
	#[serde(default)]
	pub posters: Option<Poster>,
	#[serde(default)]
	pub url: Option<String>,
	#[serde(default)]
	pub status: Option<StatusDto>,
	#[serde(default)]
	pub authors: Option<Vec<NameWrap>>,
	#[serde(default)]
	pub translators: Option<Vec<NameWrap>>,
	#[serde(default)]
	pub description: Option<String>,
	#[serde(default)]
	pub synopsis: Option<String>,
	#[serde(default)]
	pub html: Option<String>,
	#[serde(default)]
	pub tags: Option<TagsBlock>,
}

#[derive(Deserialize)]
pub struct NameWrap {
	#[serde(default)]
	pub name: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct TagsBlock {
	#[serde(default)]
	pub events: Vec<TagItem>,
	#[serde(default)]
	pub genres: Vec<TagItem>,
}

#[derive(Deserialize)]
pub struct TagItem {
	#[serde(default)]
	pub names: Option<Names>,
	#[serde(default)]
	pub title: Option<String>,
}

impl TagItem {
	fn label(&self) -> Option<String> {
		self.names
			.as_ref()
			.map(|n| n.pick(self.title.as_deref().unwrap_or_default()))
			.filter(|s| !s.is_empty())
			.or_else(|| self.title.clone().filter(|s| !s.is_empty()))
	}
}

impl DetailsItem {
	pub fn merge_into(self, mut current: Manga) -> Manga {
		current.key = self.id.to_string();
		current.title = self
			.names
			.as_ref()
			.map(|n| n.pick(&current.title))
			.filter(|s| !s.is_empty())
			.unwrap_or(current.title);
		if let Some(cover) = self.posters.as_ref().and_then(|p| p.best_url()) {
			current.cover = Some(cover);
		}
		if let Some(url) = self.url.clone().filter(|s| !s.is_empty()) {
			current.url = Some(url);
		} else {
			current.url = Some(format!("{SITE_URL}/ranobe/{}", self.id));
		}

		// Description: prefer description (longer), fall back to synopsis. Both have HTML.
		let raw = self
			.description
			.or(self.synopsis)
			.or(self.html);
		current.description = raw.map(|s| strip_html(&s)).filter(|s| !s.is_empty());

		current.status = self
			.status
			.map(|s| s.into_manga_status())
			.unwrap_or(MangaStatus::Unknown);

		let authors: Vec<String> = self
			.authors
			.unwrap_or_default()
			.into_iter()
			.filter_map(|a| a.name)
			.filter(|s| !s.is_empty())
			.collect();
		if !authors.is_empty() {
			current.authors = Some(authors);
		}

		let translators: Vec<String> = self
			.translators
			.unwrap_or_default()
			.into_iter()
			.filter_map(|t| t.name)
			.filter(|s| !s.is_empty())
			.collect();
		// Translators are surfaced as the artist field for visibility.
		if !translators.is_empty() {
			current.artists = Some(translators);
		}

		let tags = self.tags.unwrap_or_default();
		let mut all_tags: Vec<String> = Vec::new();
		for t in &tags.genres {
			if let Some(l) = t.label() {
				all_tags.push(l);
			}
		}
		for t in &tags.events {
			if let Some(l) = t.label() {
				all_tags.push(l);
			}
		}
		if !all_tags.is_empty() {
			current.tags = Some(all_tags);
		}

		current.viewer = Viewer::Vertical;
		current.content_rating = ContentRating::Safe;

		current
	}
}

// --- /api/ranobe/{id}/contents ---

#[derive(Deserialize)]
pub struct ContentsEnvelope {
	#[serde(default)]
	pub volumes: Vec<VolumeDto>,
}

#[derive(Deserialize)]
pub struct VolumeDto {
	#[serde(default)]
	pub num: Option<i32>,
	#[serde(default)]
	pub chapters: Vec<ChapterDto>,
}

#[derive(Deserialize)]
pub struct ChapterDto {
	pub id: i64,
	#[serde(default)]
	pub name: Option<String>,
	#[serde(default)]
	pub num: Option<i32>,
	#[serde(default)]
	pub url: Option<String>,
	#[serde(default)]
	pub changed_at: Option<String>,
}

impl ChapterDto {
	pub fn into_chapter(self, volume_num: Option<i32>) -> Chapter {
		let chapter_number = self.num.map(|n| n as f32);
		let volume_number = volume_num.map(|v| v as f32);
		let date_uploaded = self.changed_at.as_deref().and_then(|s| s.parse::<i64>().ok());
		let title = self.name.filter(|s| !s.trim().is_empty());
		let url = self
			.url
			.clone()
			.unwrap_or_else(|| format!("{SITE_URL}/ranobe/?/{}", self.id));
		Chapter {
			key: self.id.to_string(),
			title,
			chapter_number,
			volume_number,
			date_uploaded,
			url: Some(url),
			..Default::default()
		}
	}
}

// --- /api/chapter/{id} ---

#[derive(Deserialize)]
pub struct ChapterContentEnvelope {
	#[serde(default)]
	pub chapter: Option<ChapterContent>,
}

#[derive(Deserialize)]
pub struct ChapterContent {
	#[serde(default)]
	pub text: Option<String>,
}

// --- /api/fulltext/global?query=... ---

#[derive(Deserialize)]
pub struct FulltextSection {
	#[serde(default)]
	pub meta: Option<FulltextMeta>,
	#[serde(default)]
	pub data: Vec<FulltextItem>,
}

#[derive(Deserialize)]
pub struct FulltextMeta {
	#[serde(default)]
	pub key: Option<String>,
}

#[derive(Deserialize)]
pub struct FulltextItem {
	pub id: i64,
	#[serde(default)]
	pub names: Option<Names>,
	#[serde(default)]
	pub url: Option<String>,
	#[serde(default)]
	pub poster: Option<Poster>,
}

impl FulltextItem {
	pub fn into_manga(self) -> Manga {
		let title = self
			.names
			.as_ref()
			.map(|n| n.pick(&self.id.to_string()))
			.unwrap_or_else(|| self.id.to_string());
		let cover = self.poster.as_ref().and_then(|p| p.best_url());
		let url = self
			.url
			.clone()
			.unwrap_or_else(|| format!("{SITE_URL}/ranobe/{}", self.id));
		Manga {
			key: self.id.to_string(),
			title,
			cover,
			url: Some(url),
			..Default::default()
		}
	}
}

// --- helpers ---

/// Convert HTML chunk into plain markdown-ish text. Replaces `<p>` with double
/// newlines, drops other tags, decodes basic entities.
pub fn strip_html(html: &str) -> String {
	let mut out = String::with_capacity(html.len());
	let mut in_tag = false;
	let mut tag_buf = String::new();
	for c in html.chars() {
		if in_tag {
			if c == '>' {
				in_tag = false;
				let lower = tag_buf.to_lowercase();
				if lower == "p"
					|| lower == "/p"
					|| lower == "br"
					|| lower == "br/"
					|| lower == "br /"
				{
					if !out.ends_with("\n\n") {
						if !out.ends_with('\n') {
							out.push('\n');
						}
						out.push('\n');
					}
				}
				tag_buf.clear();
			} else {
				tag_buf.push(c);
			}
		} else if c == '<' {
			in_tag = true;
			tag_buf.clear();
		} else {
			out.push(c);
		}
	}
	// Decode minimum entities.
	let out = out
		.replace("&nbsp;", " ")
		.replace("&amp;", "&")
		.replace("&lt;", "<")
		.replace("&gt;", ">")
		.replace("&quot;", "\"")
		.replace("&#039;", "'");
	// Collapse 3+ consecutive newlines into 2.
	let mut collapsed = String::with_capacity(out.len());
	let mut newline_run = 0usize;
	for c in out.chars() {
		if c == '\n' {
			newline_run += 1;
			if newline_run <= 2 {
				collapsed.push(c);
			}
		} else {
			newline_run = 0;
			collapsed.push(c);
		}
	}
	collapsed.trim().to_string()
}
