use aidoku::imports::html::{Document, Element};
use aidoku::{Chapter, Manga, MangaStatus, Viewer};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Parse a single tile from the catalog page (`.block.story.shortstory`).
/// Tile is an `<article>` (not `<div>`) wrapping `h2.title > a` (title) and
/// `figure.cover` (cover via inline `background-image: url(...)`).
pub fn parse_tile(el: &Element) -> Option<Manga> {
	let link = el
		.select_first("h2.title a")
		.or_else(|| el.select_first(".title a"))?;
	let href = link.attr("abs:href").or_else(|| link.attr("href"))?;
	let title = link
		.text()
		.filter(|s| !s.is_empty())
		.or_else(|| link.attr("title"))
		.unwrap_or_default();
	let key = url_to_manga_key(&href)?;
	let cover = el
		.select_first("figure.cover")
		.and_then(|fig| fig.attr("style"))
		.as_deref()
		.and_then(extract_background_image)
		.or_else(|| {
			el.select_first("img")
				.and_then(|img| img.attr("abs:src").or_else(|| img.attr("src")))
		})
		.filter(|s| !s.is_empty());
	Some(Manga {
		key,
		title,
		cover,
		url: Some(href),
		..Default::default()
	})
}

/// Pull the URL out of a CSS `background-image: url(...)` declaration.
fn extract_background_image(style: &str) -> Option<String> {
	let needle = "url(";
	let start = style.find(needle)? + needle.len();
	let rest = &style[start..];
	let end = rest.find(')')?;
	let mut url = rest[..end].trim().to_string();
	// Strip optional surrounding quotes.
	if (url.starts_with('"') && url.ends_with('"'))
		|| (url.starts_with('\'') && url.ends_with('\''))
	{
		url = url[1..url.len() - 1].to_string();
	}
	if url.is_empty() { None } else { Some(url) }
}

/// Convert a manga URL like `https://ranobes.com/ranobe/12345-some-slug.html`
/// into a stable manga key `12345-some-slug`. Returns `None` if URL doesn't fit.
pub fn url_to_manga_key(url: &str) -> Option<String> {
	let after = url.split("/ranobe/").nth(1)?;
	let no_html = after.strip_suffix(".html").unwrap_or(after);
	let key = no_html.split('/').next()?;
	if key.is_empty() {
		return None;
	}
	Some(key.to_string())
}

pub fn manga_url(base_url: &str, key: &str) -> String {
	format!("{base_url}/ranobe/{key}.html")
}

/// Fill description / cover / status from the manga details page.
pub fn fill_details(doc: &Document, manga: &mut Manga) {
	if let Some(t) = doc.select_first("h1.title").and_then(|e| e.text()) {
		manga.title = t;
	}
	let cover = doc
		.select_first("meta[property=\"og:image\"]")
		.and_then(|e| e.attr("content"))
		.or_else(|| {
			doc.select_first(".poster-image img")
				.and_then(|e| e.attr("abs:src").or_else(|| e.attr("src")))
		})
		.filter(|s| !s.is_empty());
	if cover.is_some() {
		manga.cover = cover;
	}
	manga.description = doc
		.select_first("meta[property=\"og:description\"]")
		.and_then(|e| e.attr("content"))
		.or_else(|| {
			doc.select_first("meta[name=\"description\"]")
				.and_then(|e| e.attr("content"))
		})
		.filter(|s| !s.is_empty());

	let mut tags: Vec<String> = Vec::new();
	if let Some(genres) = doc.select("a[href*='/g.']") {
		for g in genres {
			if let Some(t) = g.text() {
				if !t.is_empty() {
					tags.push(t);
				}
			}
		}
	}
	if !tags.is_empty() {
		manga.tags = Some(tags);
	}

	manga.status = doc
		.select_first(".r-fullstory-spec")
		.and_then(|e| e.text())
		.map(|s| s.to_lowercase())
		.map(|s| {
			if s.contains("\u{0437}\u{0430}\u{0432}\u{0435}\u{0440}\u{0448}") {
				MangaStatus::Completed
			} else if s.contains("\u{0432}\u{044b}\u{043f}\u{0443}\u{0441}\u{043a}") {
				MangaStatus::Ongoing
			} else if s.contains("\u{0437}\u{0430}\u{043c}\u{043e}\u{0440}\u{043e}\u{0436}") {
				MangaStatus::Hiatus
			} else if s.contains("\u{043e}\u{0442}\u{043c}\u{0435}\u{043d}") {
				MangaStatus::Cancelled
			} else {
				MangaStatus::Unknown
			}
		})
		.unwrap_or(MangaStatus::Unknown);

	manga.viewer = Viewer::Vertical;
}

/// Parse the chapter slug from a chapter URL like
/// `https://ranobes.com/chapters/some-truncated-slug/`. The slug is the path
/// segment between `/chapters/` and the next `/`.
pub fn chapter_slug_from_link(href: &str) -> Option<&str> {
	let after = href.split("/chapters/").nth(1)?;
	after.split('/').next()
}

/// Parse one row of the chapter list (`.cat_block.cat_line > a`).
pub fn parse_chapter_row(el: &Element) -> Option<Chapter> {
	let link = el.select_first("a")?;
	let href = link.attr("abs:href").or_else(|| link.attr("href"))?;
	let title = link
		.attr("title")
		.or_else(|| link.select_first("h6.title").and_then(|e| e.text()))
		.filter(|s| !s.is_empty());
	let chapter_number = title.as_deref().and_then(extract_chapter_number);
	let date_uploaded = link
		.select_first("small")
		.and_then(|e| e.text())
		.as_deref()
		.and_then(parse_russian_date);
	let key = url_to_chapter_key(&href)?;
	Some(Chapter {
		key,
		title,
		chapter_number,
		date_uploaded,
		url: Some(href),
		..Default::default()
	})
}

/// Convert a chapter URL like `.../chapters/some-slug/12345-1.html` into a key
/// `some-slug/12345-1`. Reversible via `chapter_url`.
pub fn url_to_chapter_key(url: &str) -> Option<String> {
	let after = url.split("/chapters/").nth(1)?;
	let no_html = after.strip_suffix(".html").unwrap_or(after);
	if no_html.is_empty() {
		return None;
	}
	Some(no_html.to_string())
}

pub fn chapter_url(base_url: &str, key: &str) -> String {
	format!("{base_url}/chapters/{key}.html")
}

/// Pull "138" out of "Глава 138. Заказное письмо".
fn extract_chapter_number(title: &str) -> Option<f32> {
	// Find the first run of digits (with optional dot) after "Глава" or anywhere.
	let bytes = title.as_bytes();
	let mut start: Option<usize> = None;
	for (i, &b) in bytes.iter().enumerate() {
		if b.is_ascii_digit() {
			start = Some(i);
			break;
		}
	}
	let start = start?;
	let mut end = start;
	while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
		end += 1;
	}
	core::str::from_utf8(&bytes[start..end]).ok()?.parse().ok()
}

/// Parse Russian-localized date like "21 апреля 2026 в 21:37".
fn parse_russian_date(s: &str) -> Option<i64> {
	let parts: Vec<&str> = s.trim().split_whitespace().collect();
	if parts.len() < 5 {
		return None;
	}
	let day: i64 = parts[0].parse().ok()?;
	let month = russian_month(parts[1])?;
	let year: i64 = parts[2].parse().ok()?;
	// parts[3] is "в", parts[4] is "HH:MM"
	let mut hour = 0i64;
	let mut minute = 0i64;
	if let Some(time) = parts.get(4) {
		let mut hm = time.split(':');
		hour = hm.next().and_then(|s| s.parse().ok()).unwrap_or(0);
		minute = hm.next().and_then(|s| s.parse().ok()).unwrap_or(0);
	}
	let days = days_from_civil(year, month, day as u32);
	Some(days * 86400 + hour * 3600 + minute * 60)
}

fn russian_month(s: &str) -> Option<u32> {
	let lc = s.to_lowercase();
	match lc.as_str() {
		"\u{044f}\u{043d}\u{0432}\u{0430}\u{0440}\u{044f}" | "\u{044f}\u{043d}\u{0432}\u{0430}\u{0440}\u{044c}" => Some(1),
		"\u{0444}\u{0435}\u{0432}\u{0440}\u{0430}\u{043b}\u{044f}" | "\u{0444}\u{0435}\u{0432}\u{0440}\u{0430}\u{043b}\u{044c}" => Some(2),
		"\u{043c}\u{0430}\u{0440}\u{0442}\u{0430}" | "\u{043c}\u{0430}\u{0440}\u{0442}" => Some(3),
		"\u{0430}\u{043f}\u{0440}\u{0435}\u{043b}\u{044f}" | "\u{0430}\u{043f}\u{0440}\u{0435}\u{043b}\u{044c}" => Some(4),
		"\u{043c}\u{0430}\u{044f}" | "\u{043c}\u{0430}\u{0439}" => Some(5),
		"\u{0438}\u{044e}\u{043d}\u{044f}" | "\u{0438}\u{044e}\u{043d}\u{044c}" => Some(6),
		"\u{0438}\u{044e}\u{043b}\u{044f}" | "\u{0438}\u{044e}\u{043b}\u{044c}" => Some(7),
		"\u{0430}\u{0432}\u{0433}\u{0443}\u{0441}\u{0442}\u{0430}" | "\u{0430}\u{0432}\u{0433}\u{0443}\u{0441}\u{0442}" => Some(8),
		"\u{0441}\u{0435}\u{043d}\u{0442}\u{044f}\u{0431}\u{0440}\u{044f}" | "\u{0441}\u{0435}\u{043d}\u{0442}\u{044f}\u{0431}\u{0440}\u{044c}" => Some(9),
		"\u{043e}\u{043a}\u{0442}\u{044f}\u{0431}\u{0440}\u{044f}" | "\u{043e}\u{043a}\u{0442}\u{044f}\u{0431}\u{0440}\u{044c}" => Some(10),
		"\u{043d}\u{043e}\u{044f}\u{0431}\u{0440}\u{044f}" | "\u{043d}\u{043e}\u{044f}\u{0431}\u{0440}\u{044c}" => Some(11),
		"\u{0434}\u{0435}\u{043a}\u{0430}\u{0431}\u{0440}\u{044f}" | "\u{0434}\u{0435}\u{043a}\u{0430}\u{0431}\u{0440}\u{044c}" => Some(12),
		_ => None,
	}
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
	let y = if m <= 2 { y - 1 } else { y };
	let era = if y >= 0 { y } else { y - 399 } / 400;
	let yoe = (y - era * 400) as u64;
	let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) as u64 + 2) / 5 + d as u64 - 1;
	let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
	era * 146097 + doe as i64 - 719468
}

/// Extract a chapter's body as a list of (image-url | prose-text) pieces in
/// document order. The DLE template uses `<div id="arrticle">` (yes, two `r`s)
/// as the body container; inside are `<p>` paragraphs and `<img>` illustrations.
pub fn extract_chapter_pieces(doc: &Document) -> Vec<ChapterPiece> {
	let Some(article) = doc.select_first("div#arrticle") else {
		return Vec::new();
	};

	// Collect image URLs once (in document order).
	let imgs: Vec<String> = article
		.select("img")
		.map(|list| {
			list.into_iter()
				.filter_map(|img| img.attr("abs:src").or_else(|| img.attr("src")))
				.filter(|s| !s.is_empty())
				.collect()
		})
		.unwrap_or_default();

	// Collect paragraph text as a single markdown blob.
	let mut prose = String::new();
	if let Some(paragraphs) = article.select("p") {
		let mut first = true;
		for p in paragraphs {
			let text = p.text().unwrap_or_default();
			let trimmed = text.trim();
			if trimmed.is_empty() {
				continue;
			}
			if !first {
				prose.push_str("\n\n");
			}
			prose.push_str(trimmed);
			first = false;
		}
	}

	let mut pieces: Vec<ChapterPiece> = imgs.into_iter().map(ChapterPiece::Image).collect();
	if !prose.trim().is_empty() {
		pieces.push(ChapterPiece::Text(prose));
	}
	pieces
}

/// One renderable piece of a chapter body.
pub enum ChapterPiece {
	Image(String),
	Text(String),
}
