#![no_std]
extern crate alloc;

mod helpers;
mod models;
mod settings;

use crate::helpers::{
	apply_headers, base_url, fetch_chapter_pages, fetch_manga, fetch_manga_chapters, search_mangas,
};
use aidoku::imports::net::{Request, TimeUnit, set_rate_limit};
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Manga,
	MangaPageResult, Page, PageContent, PageContext, Result, Source,
	alloc::{String, Vec},
	prelude::*,
};
use alloc::string::ToString;

struct Senkuro;

impl Source for Senkuro {
	fn new() -> Self {
		set_rate_limit(2, 1, TimeUnit::Seconds);
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		_page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let q = query.unwrap_or_default();
		// Senkuro's persisted "search" requires a non-empty query.
		// If the user opens the source without typing, fall back to a popular seed string.
		let effective = if q.trim().is_empty() { "а" } else { q.as_str() };

		let nodes = search_mangas(effective)?;
		let entries: Vec<Manga> = nodes.into_iter().map(|n| n.into_manga()).collect();
		Ok(MangaPageResult {
			has_next_page: false,
			entries,
		})
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let node = fetch_manga(&manga.key)?;
		let branch_id = node.primary_branch_id();
		let base = base_url();
		let mut updated = node.into_manga(&base, needs_details);

		// keep the user-provided cover/title if we already had it and details aren't needed
		if !needs_details {
			if updated.title.is_empty() {
				updated.title = manga.title;
			}
			if updated.cover.is_none() {
				updated.cover = manga.cover;
			}
		}

		if needs_chapters {
			let mut chapters: Vec<Chapter> = Vec::new();
			if let Some(branch) = branch_id {
				let mut after: Option<String> = None;
				loop {
					let resp = fetch_manga_chapters(&branch, after.as_deref())?;
					let conn = match resp.manga_chapters {
						Some(c) => c,
						None => break,
					};
					for edge in conn.edges {
						if let Some(node) = edge.node {
							chapters.push(node.into_chapter(&base, &updated.key));
						}
					}
					match conn.page_info {
						Some(pi) if pi.has_next_page => {
							after = pi.end_cursor;
							if after.is_none() {
								break;
							}
						}
						_ => break,
					}
				}
			}
			updated.chapters = Some(chapters);
		}

		Ok(updated)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let resp = fetch_chapter_pages(&chapter.key)?;
		let pages: Vec<Page> = resp
			.manga_chapter
			.map(|c| c.pages)
			.unwrap_or_default()
			.into_iter()
			.filter_map(|p| {
				p.into_url().map(|u| Page {
					content: PageContent::url(u),
					..Page::default()
				})
			})
			.collect();
		Ok(pages)
	}
}

impl DeepLinkHandler for Senkuro {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		// Accept any senkuro.* host.
		let path = match url.find("/manga/") {
			Some(idx) => &url[idx + "/manga/".len()..],
			None => return Ok(None),
		};
		let slug = path.split('/').next().unwrap_or("");
		if slug.is_empty() {
			return Ok(None);
		}
		Ok(Some(DeepLinkResult::Manga {
			key: slug.to_string(),
		}))
	}
}

impl ImageRequestProvider for Senkuro {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(apply_headers(Request::get(url)?))
	}
}

register_source!(Senkuro, DeepLinkHandler, ImageRequestProvider);
