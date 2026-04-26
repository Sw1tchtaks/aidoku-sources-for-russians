#![no_std]
extern crate alloc;

mod graphql;
mod models;
mod settings;

use aidoku::imports::net::{Request, TimeUnit, set_rate_limit};
use aidoku::prelude::*;
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Manga,
	MangaPageResult, Page, PageContent, PageContext, Result, Source,
	alloc::{String, Vec},
};
use alloc::string::ToString;
use core::marker::PhantomData;
use serde::de::DeserializeOwned;

use graphql::{
	CHAPTERS_QUERY, DETAILS_QUERY, DetailsVariables, FiltersDto, GqlRequest, OFFSET_COUNT,
	PAGES_QUERY, PagesVariables, SEARCH_QUERY, SearchVariables,
};
use models::{
	ChaptersData, DetailsData, GqlResponse, PagesData, SearchData, build_manga_key,
	split_chapter_key, split_manga_key,
};

/// Per-source compile-time configuration consumed by [`SenkuroEngine`].
///
/// Override this trait once per Aidoku source crate. Senkuro and Senkognito share the
/// same GraphQL backend; the only differences are the public web hostname (used for
/// building URLs and deep links) and whether the built-in 18+ exclude list is applied
/// to popular/search requests.
pub trait Config: 'static {
	/// Public site name, used to detect "is this Senkuro" branching.
	const SITE: &'static str;
	/// Web base URL (no trailing slash). Used for URL fields and deep-link parsing.
	const BASE_URL: &'static str;
	/// Genre slugs that should always be excluded server-side. Applied by Senkuro to
	/// hide adult tags; Senkognito leaves this empty.
	const EXCLUDE_GENRES: &'static [&'static str] = &[];
	/// Age-rating slugs that should always be included by default in catalog/search
	/// requests (when the user hasn't picked any rating filter). Senkognito sets this
	/// to ["EXPLICIT", "QUESTIONABLE"] so the catalog actually shows the adult content
	/// the site is for; Senkuro leaves it empty so the API serves its default safe set.
	const DEFAULT_RATING_INCLUDE: &'static [&'static str] = &[];
}

pub struct SenkuroEngine<C: Config>(PhantomData<C>);

impl<C: Config> Default for SenkuroEngine<C> {
	fn default() -> Self {
		Self(PhantomData)
	}
}

impl<C: Config> Source for SenkuroEngine<C> {
	fn new() -> Self {
		set_rate_limit(3, 1, TimeUnit::Seconds);
		Self::default()
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut label = FiltersDto::default();
		let mut kind = FiltersDto::default();
		let mut format = FiltersDto::default();
		let mut status = FiltersDto::default();
		let mut translation_status = FiltersDto::default();
		let mut rating = FiltersDto::default();

		for f in filters {
			match f {
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => match id.as_str() {
					"type" => {
						kind.include.extend(included);
						kind.exclude.extend(excluded);
					}
					"format" => {
						format.include.extend(included);
						format.exclude.extend(excluded);
					}
					"status" => {
						status.include.extend(included);
						status.exclude.extend(excluded);
					}
					"translationStatus" => {
						translation_status.include.extend(included);
						translation_status.exclude.extend(excluded);
					}
					"rating" => {
						rating.include.extend(included);
						rating.exclude.extend(excluded);
					}
					"label" => {
						label.include.extend(included);
						label.exclude.extend(excluded);
					}
					_ => {}
				},
				FilterValue::Select { id, value } => match id.as_str() {
					"type" => kind.include.push(value),
					"format" => format.include.push(value),
					"status" => status.include.push(value),
					"translationStatus" => translation_status.include.push(value),
					"rating" => rating.include.push(value),
					_ => {}
				},
				_ => {}
			}
		}

		// Senkuro's permanent 18+ exclude.
		for g in C::EXCLUDE_GENRES {
			let slug: &str = g;
			if !label.exclude.iter().any(|x| x.as_str() == slug) {
				label.exclude.push(slug.to_string());
			}
		}

		// Senkognito's permanent 18+ include — only kicks in when the user
		// hasn't already picked an explicit rating filter, otherwise the
		// user's choice wins.
		if rating.include.is_empty() && rating.exclude.is_empty() {
			for r in C::DEFAULT_RATING_INCLUDE {
				let slug: &str = r;
				if !rating.include.iter().any(|x| x.as_str() == slug) {
					rating.include.push(slug.to_string());
				}
			}
		}

		let vars = SearchVariables {
			query: query.filter(|q| !q.trim().is_empty()),
			kind: kind.into_option(),
			status: status.into_option(),
			translation_status: translation_status.into_option(),
			label: label.into_option(),
			format: format.into_option(),
			rating: rating.into_option(),
			offset: Some(OFFSET_COUNT * (page - 1).max(0)),
		};

		let payload = GqlRequest {
			query: SEARCH_QUERY,
			variables: vars,
		};
		let body = serde_json::to_vec(&payload).map_err(|e| error!("encode search: {e}"))?;
		let data: SearchData = post_graphql("searchTachiyomiManga", &body)?;
		let mangas = data
			.manga_tachiyomi_search
			.map(|p| p.mangas)
			.unwrap_or_default();
		let has_next_page = mangas.len() as i32 >= OFFSET_COUNT;
		let entries: Vec<Manga> = mangas.into_iter().map(|m| m.into_manga(C::BASE_URL)).collect();
		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let (manga_id, slug) = {
			let (id, slug) = split_manga_key(&manga.key);
			(id.to_string(), slug.to_string())
		};

		let mut updated = manga;

		if needs_details {
			let body = serde_json::to_vec(&GqlRequest {
				query: DETAILS_QUERY,
				variables: DetailsVariables {
					manga_id: &manga_id,
				},
			})
			.map_err(|e| error!("encode details: {e}"))?;
			let data: DetailsData = post_graphql("fetchTachiyomiManga", &body)?;
			let info = data
				.manga_tachiyomi_info
				.ok_or_else(|| error!("manga \"{}\" not found", slug))?;
			let mut detailed = info.into_manga(C::BASE_URL);
			// Preserve key in the canonical form already stored by the app.
			detailed.key = build_manga_key(&manga_id, &slug);
			// Carry over chapters if we already had them.
			detailed.chapters = updated.chapters.take();
			updated = detailed;
		}

		if needs_chapters {
			let body = serde_json::to_vec(&GqlRequest {
				query: CHAPTERS_QUERY,
				variables: DetailsVariables {
					manga_id: &manga_id,
				},
			})
			.map_err(|e| error!("encode chapters: {e}"))?;
			let data: ChaptersData = post_graphql("fetchTachiyomiChapters", &body)?;
			let payload = data.manga_tachiyomi_chapters.unwrap_or_default();
			let teams = payload.teams;
			let chapters: Vec<Chapter> = payload
				.chapters
				.into_iter()
				.map(|c| c.into_chapter(C::BASE_URL, &slug, &teams))
				.collect();
			updated.chapters = Some(chapters);
		}

		Ok(updated)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let (manga_id, _) = split_manga_key(&manga.key);
		let (chapter_id, _) = split_chapter_key(&chapter.key);
		let body = serde_json::to_vec(&GqlRequest {
			query: PAGES_QUERY,
			variables: PagesVariables {
				manga_id,
				chapter_id,
			},
		})
		.map_err(|e| error!("encode pages: {e}"))?;
		let data: PagesData = post_graphql("fetchTachiyomiChapterPages", &body)?;
		let pages = data
			.manga_tachiyomi_chapter_pages
			.map(|p| p.pages)
			.unwrap_or_default();
		Ok(pages
			.into_iter()
			.map(|p| Page {
				content: PageContent::url(p.url),
				..Default::default()
			})
			.collect())
	}
}

impl<C: Config> DeepLinkHandler for SenkuroEngine<C> {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		// Accept any senkuro.* / senkognito.* host; just look for /manga/{slug}.
		let Some(idx) = url.find("/manga/") else {
			return Ok(None);
		};
		let rest = &url[idx + "/manga/".len()..];
		let slug = rest.split('/').next().unwrap_or("");
		if slug.is_empty() {
			return Ok(None);
		}
		// We don't know the manga ID without an API call. Use slug alone as the key
		// suffix, leaving the prefix empty — split_manga_key handles single-token keys
		// by returning (key, key). The first details fetch will then fail because the
		// API needs an ID; in practice users open mangas through the catalog where the
		// key is already in `id,,slug` form, so this fallback only matters for direct
		// shared links.
		Ok(Some(DeepLinkResult::Manga {
			key: alloc::format!(",,{}", slug),
		}))
	}
}

impl<C: Config> ImageRequestProvider for SenkuroEngine<C> {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		let req = Request::get(url)?
			.header(
				"User-Agent",
				"Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
			)
			.header("Referer", C::BASE_URL);
		Ok(req)
	}
}

fn post_graphql<T: DeserializeOwned>(operation: &str, body: &[u8]) -> Result<T> {
	let url = settings::api_url();
	let response = Request::post(&url)?
		.header("Content-Type", "application/json")
		.header("Accept", "application/json")
		.header(
			"User-Agent",
			"Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
		)
		.body(body)
		.send()?;

	let status = response.status_code();
	let bytes = response.get_data()?;
	if !(200..300).contains(&status) {
		let preview = preview_body(&bytes);
		println!("[senkuro:{operation}] HTTP {status}: {preview}");
		return Err(error!("Senkuro {operation} HTTP {status}"));
	}

	let raw: GqlResponse<T> = match serde_json::from_slice(&bytes) {
		Ok(v) => v,
		Err(e) => {
			let preview = preview_body(&bytes);
			println!("[senkuro:{operation}] parse error: {e}, body: {preview}");
			return Err(error!("Senkuro {operation} parse error: {e}"));
		}
	};
	if let Some(errors) = raw.errors {
		let joined = errors
			.into_iter()
			.map(|e| e.message)
			.collect::<Vec<_>>()
			.join("; ");
		println!("[senkuro:{operation}] GraphQL errors: {joined}");
		return Err(error!("Senkuro {operation}: {joined}"));
	}
	raw.data.ok_or_else(|| {
		let preview = preview_body(&bytes);
		println!("[senkuro:{operation}] empty data, body: {preview}");
		error!("Senkuro {operation}: empty data")
	})
}

fn preview_body(bytes: &[u8]) -> String {
	let limit = bytes.len().min(400);
	String::from_utf8_lossy(&bytes[..limit]).into_owned()
}
