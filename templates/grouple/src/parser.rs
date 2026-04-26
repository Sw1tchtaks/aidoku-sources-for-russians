use aidoku::imports::html::{Document, Element};
use aidoku::{Chapter, Manga, MangaStatus};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Parse a single tile from the catalog/search list (`div.tile`).
pub fn parse_tile(el: &Element, base_url: &str) -> Option<Manga> {
	let link = el.select_first("h3 > a")?;
	let href = link.attr("href")?;
	let title = link.attr("title").unwrap_or_else(|| link.text().unwrap_or_default());
	let cover = el
		.select_first("img.lazy")
		.and_then(|img| img.attr("data-original"))
		.map(|s| s.replace("_p.", "."));

	let key = strip_domain(&href);
	if key.is_empty() {
		return None;
	}

	Some(Manga {
		key,
		title,
		cover,
		url: Some(absolute_url(&href, base_url)),
		..Default::default()
	})
}

/// Strip the protocol+host from an URL, leaving an absolute path like `/some/slug`.
pub fn strip_domain(url: &str) -> String {
	if let Some(after_proto) = url.split_once("://") {
		let after_host = after_proto.1.split_once('/');
		match after_host {
			Some((_, path)) => format!("/{path}"),
			None => "/".to_string(),
		}
	} else {
		url.to_string()
	}
}

pub fn absolute_url(path: &str, base_url: &str) -> String {
	if path.starts_with("http://") || path.starts_with("https://") {
		path.to_string()
	} else if let Some(p) = path.strip_prefix("//") {
		format!("https://{p}")
	} else if path.starts_with('/') {
		format!("{base_url}{path}")
	} else {
		format!("{base_url}/{path}")
	}
}

/// Parse manga details. Tries the new "cr-*" layout (Readmanga's modern theme)
/// first, then falls back to the legacy `.expandable` layout used by older
/// Grouple sites and AllHentai.
pub fn parse_details(doc: &Document, base_url: &str, key: &str, mut current: Manga) -> Manga {
	current.key = key.to_string();
	current.url = Some(format!("{base_url}{key}"));

	let modern = doc.select_first(".cr-hero-names__main");
	if modern.is_some() {
		fill_modern(doc, &mut current);
	} else {
		fill_legacy(doc, &mut current);
	}

	current
}

fn fill_modern(doc: &Document, manga: &mut Manga) {
	if let Some(t) = doc.select_first(".cr-hero-names__main").and_then(|e| e.text()) {
		manga.title = t;
	}

	// people
	let mut authors = Vec::new();
	let mut artists = Vec::new();
	if let Some(people) = doc.select(".cr-main-person-item") {
		for person in people {
			let role = person
				.select_first(".cr-main-person-item__role")
				.and_then(|e| e.text())
				.unwrap_or_default()
				.to_lowercase();
			let name = person
				.select_first(".cr-main-person-item__name a")
				.and_then(|e| e.text())
				.or_else(|| {
					person
						.select_first(".cr-main-person-item__name")
						.and_then(|e| e.text())
				})
				.unwrap_or_default();
			if name.is_empty() {
				continue;
			}
			if role.contains("\u{0430}\u{0432}\u{0442}\u{043e}\u{0440}")
				|| role.contains("\u{0441}\u{0446}\u{0435}\u{043d}\u{0430}\u{0440}")
			{
				authors.push(name);
			} else if role.contains("\u{0445}\u{0443}\u{0434}\u{043e}\u{0436}")
				|| role.contains("\u{0438}\u{043b}\u{043b}\u{044e}\u{0441}\u{0442}")
			{
				artists.push(name);
			}
		}
	}
	if !authors.is_empty() {
		manga.authors = Some(authors);
	}
	if !artists.is_empty() {
		manga.artists = Some(artists);
	}

	// tags
	let mut tags: Vec<String> = Vec::new();
	if let Some(items) = doc.select(".cr-tags .cr-tags__item") {
		for tag in items {
			if let Some(span) = tag.select("span").and_then(|list| list.last()) {
				if let Some(text) = span.text() {
					if !text.is_empty() {
						tags.push(text);
					}
				}
			}
		}
	}
	if !tags.is_empty() {
		manga.tags = Some(tags);
	}

	// description
	manga.description = doc
		.select_first(".cr-description__content")
		.and_then(|e| e.text());

	// cover
	let cover = doc
		.select_first(".cr-hero-poster__img")
		.or_else(|| doc.select_first(".cr-hero-overlay__bg"))
		.and_then(|el| {
			el.attr("src")
				.or_else(|| el.attr("data-src"))
				.or_else(|| el.attr("data-original"))
				.or_else(|| el.attr("data-bg"))
		});
	if cover.is_some() {
		manga.cover = cover;
	}

	// status
	let mut release = String::new();
	let mut translation = String::new();
	if let Some(items) = doc.select(".cr-info-details__item") {
		for item in items {
			let title = item
				.select_first(".cr-info-details__title")
				.and_then(|e| e.text())
				.unwrap_or_default()
				.to_lowercase();
			let value = item
				.select_first(".cr-info-details__content")
				.and_then(|e| e.text())
				.unwrap_or_default()
				.to_lowercase();
			if title.contains("\u{0432}\u{044b}\u{043f}\u{0443}\u{0441}\u{043a}") {
				release = value;
			} else if title.contains("\u{043f}\u{0435}\u{0440}\u{0435}\u{0432}\u{043e}\u{0434}") {
				translation = value;
			}
		}
	}
	manga.status = grouple_status(&release, &translation);
}

fn fill_legacy(doc: &Document, manga: &mut Manga) {
	if let Some(t) = doc.select_first(".names > .name").and_then(|e| e.text()) {
		manga.title = t;
	}

	let info = doc.select_first(".expandable");

	if let Some(info) = info.as_ref() {
		manga.authors = info
			.select_first("span.elem_author")
			.or_else(|| info.select_first("span.elem_screenwriter"))
			.and_then(|e| e.text())
			.map(|s| Vec::from([s]));
		manga.artists = info
			.select_first("span.elem_illustrator")
			.and_then(|e| e.text())
			.map(|s| Vec::from([s]));

		// tags from genre/tag links
		let mut tags = Vec::new();
		if let Some(genres) = info.select("a[href*=\"/list/genre/\"]") {
			for g in genres {
				if let Some(t) = g.text() {
					tags.push(t);
				}
			}
		}
		if let Some(more_tags) = info.select("a[href*=\"/list/tag/\"]") {
			for g in more_tags {
				if let Some(t) = g.text() {
					tags.push(t);
				}
			}
		}
		if !tags.is_empty() {
			manga.tags = Some(tags);
		}

		// status: look at badge text
		let badges = info
			.select("span.badge")
			.map(|list| {
				let mut buf = String::new();
				for b in list {
					if let Some(t) = b.text() {
						buf.push_str(&t);
						buf.push(' ');
					}
				}
				buf.to_lowercase()
			})
			.unwrap_or_default();

		manga.status = if badges.contains("\u{043f}\u{0440}\u{043e}\u{0434}\u{043e}\u{043b}\u{0436}")
			|| badges.contains("\u{043d}\u{0430}\u{0447}\u{0430}\u{0442}")
		{
			MangaStatus::Ongoing
		} else if badges.contains("\u{0437}\u{0430}\u{0432}\u{0435}\u{0440}\u{0448}") {
			MangaStatus::Completed
		} else if badges.contains("\u{043f}\u{0440}\u{0438}\u{043e}\u{0441}\u{0442}")
			|| badges.contains("\u{0437}\u{0430}\u{043c}\u{043e}\u{0440}\u{043e}\u{0436}")
		{
			MangaStatus::Hiatus
		} else {
			MangaStatus::Unknown
		};

		// cover
		let cover = info.select_first("img").and_then(|img| {
			img.attr("data-full")
				.or_else(|| img.attr("data-original"))
				.or_else(|| img.attr("src"))
		});
		if cover.is_some() {
			manga.cover = cover;
		}
	}

	manga.description = doc
		.select_first("div#tab-description .manga-description")
		.and_then(|e| e.text());
}

fn grouple_status(release: &str, translation: &str) -> MangaStatus {
	if release.contains("\u{043f}\u{0440}\u{043e}\u{0434}\u{043e}\u{043b}\u{0436}")
		|| release.contains("\u{043d}\u{0430}\u{0447}\u{0430}\u{0442}")
	{
		MangaStatus::Ongoing
	} else if release.contains("\u{0437}\u{0430}\u{0432}\u{0435}\u{0440}\u{0448}") {
		if translation.contains("\u{0437}\u{0430}\u{0432}\u{0435}\u{0440}\u{0448}") {
			MangaStatus::Completed
		} else {
			// "publishing finished" — Aidoku doesn't have this distinction; use Completed.
			MangaStatus::Completed
		}
	} else if release.contains("\u{043f}\u{0440}\u{0438}\u{043e}\u{0441}\u{0442}")
		|| release.contains("\u{0437}\u{0430}\u{043c}\u{043e}\u{0440}\u{043e}\u{0436}")
	{
		MangaStatus::Hiatus
	} else {
		MangaStatus::Unknown
	}
}

/// Extract the user_hash value embedded in the chapter list page script.
/// Returns the "?d=HASH&mtr=true" suffix, or "?mtr=true" when not present.
pub fn extract_chapter_query(doc: &Document) -> String {
	let scripts = doc.select("script");
	if let Some(scripts) = scripts {
		for script in scripts {
			let data = script.data().unwrap_or_default();
			if !data.contains("user_hash") {
				continue;
			}
			if let Some(idx) = data.find("user_hash") {
				let rest = &data[idx..];
				let mut chars = rest.chars().peekable();
				while let Some(c) = chars.next() {
					if c == '\'' || c == '"' {
						let quote = c;
						let mut hash = String::new();
						for inner in chars.by_ref() {
							if inner == quote {
								return format!("?d={hash}&mtr=true");
							}
							hash.push(inner);
						}
						break;
					}
				}
			}
		}
	}
	"?mtr=true".to_string()
}

/// Parse a single chapter row.
pub fn parse_chapter_row(
	el: &Element,
	_manga_key: &str,
	query: &str,
	base_url: &str,
) -> Option<Chapter> {
	let link = el.select_first("a.chapter-link")?;
	let href = link.attr("href")?;
	let raw_path = strip_domain(&href);
	let key = format!("{raw_path}{query}");
	let url = absolute_url(&href, base_url);

	let item_title = el.select_first("td.item-title");
	let chapter_number = item_title
		.as_ref()
		.and_then(|e| e.attr("data-num"))
		.and_then(|s| s.parse::<f32>().ok())
		.map(|n| n / 10.0);

	let mut name = link.text().unwrap_or_default();
	name = name.trim_end_matches(" \u{043d}\u{043e}\u{0432}\u{043e}\u{0435}").trim().to_string();

	let date_uploaded = el
		.select("td.d-none")
		.and_then(|list| list.last())
		.and_then(|e| e.text())
		.and_then(|s| parse_dot_date(&s));

	let scanlator = link
		.attr("title")
		.map(|t| {
			t.replace("(\u{041f}\u{0435}\u{0440}\u{0435}\u{0432}\u{043e}\u{0434}\u{0447}\u{0438}\u{043a}),", "&")
				.replace(" (\u{041f}\u{0435}\u{0440}\u{0435}\u{0432}\u{043e}\u{0434}\u{0447}\u{0438}\u{043a})", "")
		})
		.filter(|s| !s.is_empty());
	let scanlators = scanlator.map(|s| Vec::from([s]));

	Some(Chapter {
		key,
		title: if name.is_empty() { None } else { Some(name) },
		chapter_number,
		date_uploaded,
		scanlators,
		url: Some(url),
		..Default::default()
	})
}

/// Parse `dd.MM.yy` or `dd/MM/yy` into unix seconds (UTC midnight).
fn parse_dot_date(s: &str) -> Option<i64> {
	let parts: Vec<&str> = s.split(|c: char| c == '.' || c == '/').collect();
	if parts.len() != 3 {
		return None;
	}
	let day: i64 = parts[0].trim().parse().ok()?;
	let month: i64 = parts[1].trim().parse().ok()?;
	let mut year: i64 = parts[2].trim().parse().ok()?;
	if year < 100 {
		year += 2000;
	}
	let days = days_from_civil(year, month as u32, day as u32);
	Some(days * 86400)
}

// Howard Hinnant's days_from_civil algorithm.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
	let y = if m <= 2 { y - 1 } else { y };
	let era = if y >= 0 { y } else { y - 399 } / 400;
	let yoe = (y - era * 400) as u64;
	let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) as u64 + 2) / 5 + d as u64 - 1;
	let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
	era * 146097 + doe as i64 - 719468
}
