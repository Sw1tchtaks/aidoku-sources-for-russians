#![no_std]
extern crate alloc;

mod filters;
mod graphql;
mod models;
mod settings;

use aidoku::imports::defaults::{DefaultValue, defaults_get, defaults_set};
use aidoku::imports::net::{Request, TimeUnit, set_rate_limit};
use aidoku::prelude::*;
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter, FilterValue, Home,
	HomeComponent, HomeComponentValue, HomeLayout, ImageRequestProvider, Link, Listing,
	ListingKind, ListingProvider, Manga, MangaPageResult, Page, PageContent, PageContext, Result,
	Source,
	alloc::{String, Vec},
};
use alloc::string::ToString;
use core::marker::PhantomData;
use serde::Serialize;
use serde::de::DeserializeOwned;

use graphql::{
	CHAPTERS_QUERY, DETAILS_QUERY, DateRangeDto, DetailsVariables, FILTERS_QUERY, FiltersDto,
	GqlRequest, LATEST_TITLES_QUERY, LATEST_UPDATES_QUERY, MANGAS_QUERY,
	MangaCatalogVariables, MangaConnectionVariables, MangasVariables, NumberRangeDto, PAGE_SIZE,
	PAGES_QUERY, POPULAR_BY_PERIOD_QUERY, PagesVariables, PeriodVariables, TOP_BY_TYPE_QUERY,
	WEB_MANGAS_QUERY,
};
use models::{
	ChaptersData, DetailsData, FiltersResponse, GqlResponse, MangaConnectionData, MangasData,
	PagesData, PopularByPeriodData, build_manga_key, split_chapter_key, split_manga_key,
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
	/// GraphQL hosts and public application id used by the corresponding website.
	const DEFAULT_API_DOMAIN: &'static str = "https://api.senkuro.com";
	const FALLBACK_API_DOMAIN: &'static str = "https://api.senkuro.me";
	const APP_ID: &'static str = "4026531840100";
	/// Genre slugs that should always be excluded server-side. Applied by Senkuro to
	/// hide adult tags; Senkognito leaves this empty.
	const EXCLUDE_GENRES: &'static [&'static str] = &[];
	/// Age-rating slugs that should always be included by default in catalog/search
	/// requests when the user hasn't picked a rating filter. Senkognito uses EXPLICIT;
	/// Senkuro leaves this empty.
	const DEFAULT_RATING_INCLUDE: &'static [&'static str] = &[];
	/// Label slugs that are required for every catalog/search request.
	const DEFAULT_LABEL_INCLUDE: &'static [&'static str] = &[];
	/// Whether the generic comics section should be shown on the home page.
	const INCLUDE_COMICS: bool = true;
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
		let mut source = FiltersDto::default();
		let mut origin_country = FiltersDto::default();
		let mut chapters: Option<NumberRangeDto> = None;
		let mut released_on: Option<DateRangeDto> = None;
		let mut order_field = "POPULARITY_SCORE";
		let mut order_direction = "DESC";

		for value in C::DEFAULT_LABEL_INCLUDE {
			label.include.push((*value).to_string());
		}

		for f in filters {
			match f {
				FilterValue::Sort {
					id,
					index,
					ascending,
				} if id == "sort" => {
					order_field = match index {
						1 => "SCORE",
						2 => "CHAPTERS",
						3 => "VIEWS",
						4 => "CREATED_AT",
						5 => "LAST_CHAPTER_AT",
						_ => "POPULARITY_SCORE",
					};
					order_direction = if ascending { "ASC" } else { "DESC" };
				}
				FilterValue::Range { id, from, to } => match id.as_str() {
					"chapters" => {
						chapters = number_range(from, to);
					}
					"releasedOn" => {
						released_on = year_range(from, to);
					}
					_ => {}
				},
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} => {
					if id.starts_with("label") {
						// Genre groups: dynamic filter ids look like "label_TEFCRUw6NQ".
						label.include.extend(included);
						label.exclude.extend(excluded);
					} else {
						match id.as_str() {
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
							"source" => {
								source.include.extend(included);
								source.exclude.extend(excluded);
							}
							"originCountry" => {
								origin_country.include.extend(included);
								origin_country.exclude.extend(excluded);
							}
							_ => {}
						}
					}
				}
				FilterValue::Select { id, value } => match id.as_str() {
					"type" => kind.include.push(value),
					"format" => format.include.push(value),
					"status" => status.include.push(value),
					"translationStatus" => translation_status.include.push(value),
					"rating" => rating.include.push(value),
					"source" => source.include.push(value),
					"originCountry" => origin_country.include.push(value),
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

		let trimmed_query = query
			.as_ref()
			.map(|q| q.trim().to_string())
			.filter(|q| !q.is_empty());

		let vars = MangaCatalogVariables {
			first: PAGE_SIZE,
			after: None,
			search: trimmed_query,
			kind: kind.into_option(),
			format: format.into_option(),
			status: status.into_option(),
			source: source.into_option(),
			rating: rating.into_option(),
			chapters,
			origin_country: origin_country.into_option(),
			released_on,
			label: label.into_option(),
			translation_status: translation_status.into_option(),
			order_field,
			order_direction,
		};
		Self::fetch_web_catalog(vars, page)
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
			let data: DetailsData = post_graphql::<C, DetailsData>("fetchTachiyomiManga", &body)?;
			let info = data
				.manga_tachiyomi_info
				.ok_or_else(|| error!("manga \"{}\" not found", slug))?;
			let mut detailed = info.into_manga(C::BASE_URL);
			// Preserve key in the canonical form already stored by the app.
			detailed.key = build_manga_key(&manga_id, &slug);
			// Keep the cover users saw in the list. Senkuro may expose a different
			// historical cover through its Tachiyomi detail endpoint.
			if updated.cover.is_some() {
				detailed.cover = updated.cover.take();
			}
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
			let data: ChaptersData =
				post_graphql::<C, ChaptersData>("fetchTachiyomiChapters", &body)?;
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
		let data: PagesData = post_graphql::<C, PagesData>("fetchTachiyomiChapterPages", &body)?;
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
		let body = serde_json::to_vec(&GqlRequest {
			query: MANGAS_QUERY,
			variables: MangasVariables {
				search: Some(slug.to_string()),
				kind: None,
				status: None,
				translation_status: None,
				label: default_label::<C>(),
				format: None,
				rating: default_rating::<C>(),
				offset: Some(0),
			},
		})
		.map_err(|e| error!("encode deep link search: {e}"))?;
		let data: MangasData = post_graphql::<C, MangasData>("resolveDeepLink", &body)?;
		let manga = data
			.manga_tachiyomi_search
			.unwrap_or_default()
			.mangas
			.into_iter()
			.find(|manga| manga.slug == slug);
		Ok(manga.map(|manga| DeepLinkResult::Manga {
			key: build_manga_key(&manga.id, &manga.slug),
		}))
	}
}

impl<C: Config> SenkuroEngine<C> {
	/// Build a current web-catalog request with at most a single type filter.
	/// Used by both [`ListingProvider`] tabs and home layout sections.
	fn fetch_catalog(
		_listing_id: &str,
		type_slug: Option<&'static str>,
		page: i32,
	) -> Result<MangaPageResult> {
		let mut kind = FiltersDto::default();
		if let Some(t) = type_slug {
			kind.include.push(t.to_string());
		}

		let vars = MangaCatalogVariables {
			first: PAGE_SIZE,
			after: None,
			search: None,
			kind: kind.into_option(),
			format: None,
			status: None,
			source: None,
			rating: default_rating::<C>(),
			chapters: None,
			origin_country: None,
			released_on: None,
			label: default_label::<C>(),
			translation_status: None,
			order_field: "POPULARITY_SCORE",
			order_direction: "DESC",
		};
		Self::fetch_web_catalog(vars, page)
	}

	fn fetch_web_catalog(
		mut vars: MangaCatalogVariables,
		page: i32,
	) -> Result<MangaPageResult> {
		let page = page.max(1);
		vars.after = None;
		vars.first = PAGE_SIZE;
		let signature_body = serde_json::to_vec(&GqlRequest {
			query: WEB_MANGAS_QUERY,
			variables: &vars,
		})
		.map_err(|e| error!("encode catalog signature: {e}"))?;
		let signature = fnv1a64(&signature_body);

		let mut skip = 0usize;
		if page > 1 {
			let key = cursor_key(signature, page);
			if let Some(cursor) = defaults_get::<String>(&key) {
				vars.after = Some(cursor);
			} else {
				// Aidoku normally requests pages in order. For a direct jump, recover
				// without a cursor while the API's 100-record limit allows it.
				let cumulative = PAGE_SIZE.saturating_mul(page);
				if cumulative > 100 {
					return Ok(MangaPageResult {
						entries: Vec::new(),
						has_next_page: false,
					});
				}
				vars.first = cumulative;
				skip = (PAGE_SIZE.saturating_mul(page - 1)) as usize;
			}
		}

		let body = serde_json::to_vec(&GqlRequest {
			query: WEB_MANGAS_QUERY,
			variables: vars,
		})
		.map_err(|e| error!("encode web catalog: {e}"))?;
		let data: MangaConnectionData =
			post_graphql::<C, MangaConnectionData>("fetchMangas", &body)?;
		let connection = data.mangas.unwrap_or_default();
		if let Some(cursor) = connection.page_info.end_cursor.clone() {
			defaults_set(
				&cursor_key(signature, page + 1),
				DefaultValue::String(cursor),
			);
		}
		let entries = connection
			.edges
			.into_iter()
			.skip(skip)
			.take(PAGE_SIZE as usize)
			.map(|edge| edge.node.into_manga(C::BASE_URL))
			.collect();
		Ok(MangaPageResult {
			entries,
			has_next_page: connection.page_info.has_next_page,
		})
	}

	fn fetch_popular_period(period: &'static str) -> Result<Vec<Manga>> {
		let body = serde_json::to_vec(&GqlRequest {
			query: POPULAR_BY_PERIOD_QUERY,
			variables: PeriodVariables { period },
		})
		.map_err(|e| error!("encode popular period: {e}"))?;
		let data: PopularByPeriodData =
			post_graphql::<C, PopularByPeriodData>("fetchMangaPopularByPeriod", &body)?;
		Ok(data
			.manga_popular_by_period
			.into_iter()
			.map(|m| m.into_manga(C::BASE_URL))
			.collect())
	}

	fn fetch_manga_connection(
		query: &'static str,
		operation: &str,
		type_slug: Option<&'static str>,
		first: i32,
	) -> Result<Vec<Manga>> {
		let mut kind = FiltersDto::default();
		if let Some(t) = type_slug {
			kind.include.push(t.to_string());
		}
		let body = serde_json::to_vec(&GqlRequest {
			query,
			variables: MangaConnectionVariables {
				first,
				kind: kind.into_option(),
				label: default_label::<C>(),
				rating: default_rating::<C>(),
			},
		})
		.map_err(|e| error!("encode {operation}: {e}"))?;
		let data: MangaConnectionData = post_graphql::<C, MangaConnectionData>(operation, &body)?;
		Ok(data
			.mangas
			.unwrap_or_default()
			.edges
			.into_iter()
			.map(|edge| edge.node.into_manga(C::BASE_URL))
			.collect())
	}

	fn fetch_static_listing(
		query: &'static str,
		operation: &str,
		type_slug: Option<&'static str>,
		page: i32,
	) -> Result<MangaPageResult> {
		if page > 1 {
			return Ok(MangaPageResult {
				entries: Vec::new(),
				has_next_page: false,
			});
		}
		Ok(MangaPageResult {
			entries: Self::fetch_manga_connection(query, operation, type_slug, 24)?,
			has_next_page: false,
		})
	}

	fn get_catalog_home(&self) -> Result<HomeLayout> {
		let popular = Self::fetch_catalog("popular", None, 1)?.entries;
		let featured: Vec<Manga> = popular
			.iter()
			.take(HOME_FEATURED_COUNT)
			.cloned()
			.map(|m| self.get_manga_update(m.clone(), true, false).unwrap_or(m))
			.collect();
		let more_popular: Vec<Link> = popular
			.iter()
			.skip(HOME_FEATURED_COUNT)
			.cloned()
			.map(Link::from)
			.collect();

		let latest_updates = Self::fetch_manga_connection(
			LATEST_UPDATES_QUERY,
			"fetchLatestMangaUpdates",
			None,
			HOME_SCROLLER_COUNT,
		)
		.unwrap_or_default();
		let latest_titles = Self::fetch_manga_connection(
			LATEST_TITLES_QUERY,
			"fetchLatestMangaTitles",
			None,
			HOME_SCROLLER_COUNT,
		)
		.unwrap_or_default();

		let mut components: Vec<HomeComponent> = Vec::with_capacity(4 + TYPE_SECTIONS.len());
		components.push(HomeComponent {
			title: Some("Популярное".to_string()),
			subtitle: Some("За всё время".to_string()),
			value: HomeComponentValue::BigScroller {
				entries: featured,
				auto_scroll_interval: Some(8.0),
			},
		});
		if !more_popular.is_empty() {
			components.push(HomeComponent {
				title: Some("Ещё популярное".to_string()),
				subtitle: Some("Продолжение подборки".to_string()),
				value: HomeComponentValue::Scroller {
					entries: more_popular,
					listing: Some(Listing {
						id: "popular".to_string(),
						name: "Популярное".to_string(),
						kind: ListingKind::Default,
					}),
				},
			});
		}
		push_scroller(
			&mut components,
			"latest_updates",
			"Последние обновления",
			latest_updates,
		);
		push_scroller(
			&mut components,
			"latest_titles",
			"Новые тайтлы",
			latest_titles,
		);
		for (lid, title, type_slug) in TYPE_SECTIONS {
			if *lid == "comics" && !C::INCLUDE_COMICS {
				continue;
			}
			let entries = Self::fetch_catalog(*lid, *type_slug, 1)
				.map(|r| r.entries)
				.unwrap_or_default();
			if entries.is_empty() {
				continue;
			}
			let links: Vec<Link> = entries.into_iter().map(Link::from).collect();
			components.push(HomeComponent {
				title: Some((*title).to_string()),
				subtitle: section_subtitle(lid),
				value: HomeComponentValue::Scroller {
					entries: links,
					listing: Some(Listing {
						id: (*lid).to_string(),
						name: (*title).to_string(),
						kind: ListingKind::Default,
					}),
				},
			});
		}
		Ok(HomeLayout { components })
	}
}

fn default_label<C: Config>() -> Option<FiltersDto> {
	let mut label = FiltersDto::default();
	for value in C::DEFAULT_LABEL_INCLUDE {
		label.include.push((*value).to_string());
	}
	for g in C::EXCLUDE_GENRES {
		label.exclude.push((*g).to_string());
	}
	label.into_option()
}

fn default_rating<C: Config>() -> Option<FiltersDto> {
	let mut rating = FiltersDto::default();
	for r in C::DEFAULT_RATING_INCLUDE {
		rating.include.push((*r).to_string());
	}
	rating.into_option()
}

fn number_range(from: Option<f32>, to: Option<f32>) -> Option<NumberRangeDto> {
	let start = from.map(|value| value as i32).filter(|value| *value > 0);
	let end = to
		.map(|value| value as i32)
		.filter(|value| *value < 999);
	if start.is_none() && end.is_none() {
		None
	} else {
		Some(NumberRangeDto { start, end })
	}
}

fn year_range(from: Option<f32>, to: Option<f32>) -> Option<DateRangeDto> {
	let start = from
		.map(|value| value as i32)
		.filter(|value| *value > 1970)
		.map(|year| alloc::format!("{year}-01-01"));
	let end = to
		.map(|value| value as i32)
		.filter(|value| *value < 2030)
		.map(|year| alloc::format!("{year}-01-01"));
	if start.is_none() && end.is_none() {
		None
	} else {
		Some(DateRangeDto { start, end })
	}
}

fn fnv1a64(bytes: &[u8]) -> u64 {
	let mut hash = 0xcbf29ce484222325u64;
	for byte in bytes {
		hash ^= *byte as u64;
		hash = hash.wrapping_mul(0x100000001b3);
	}
	hash
}

fn cursor_key(signature: u64, page: i32) -> String {
	alloc::format!("senkuro.catalog.{signature:016x}.{page}")
}

const TYPE_SECTIONS: &[(&str, &str, Option<&str>)] = &[
	// (listing_id, display_title, optional type-filter slug)
	("manga", "Манга", Some("MANGA")),
	("manhwa", "Манхва", Some("MANHWA")),
	("manhua", "Маньхуа", Some("MANHUA")),
	("comics", "Комиксы", Some("COMICS")),
];

const HOME_FEATURED_COUNT: usize = 3;
const HOME_SCROLLER_COUNT: i32 = 12;

fn section_subtitle(id: &str) -> Option<String> {
	match id {
		"popular" => Some("За день".to_string()),
		"top_week" => Some("За неделю".to_string()),
		"top_month" => Some("За месяц".to_string()),
		"latest_updates" => Some("Свежие главы".to_string()),
		"latest_titles" => Some("Новые тайтлы".to_string()),
		"top_manhwa" => Some("По оценкам читателей".to_string()),
		"manga" => Some("Японские тайтлы".to_string()),
		"manhwa" => Some("Корейские вебтуны".to_string()),
		"manhua" => Some("Китайские тайтлы".to_string()),
		"comics" => Some("Комиксы и OEL".to_string()),
		_ => None,
	}
}

impl<C: Config> ListingProvider for SenkuroEngine<C> {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let id = listing.id.clone();
		let id_ref = id.as_str();
		if id_ref == "popular" || id_ref.is_empty() {
			if !C::DEFAULT_RATING_INCLUDE.is_empty() {
				return Self::fetch_catalog("popular", None, page);
			}
			if page > 1 {
				return Ok(MangaPageResult {
					entries: Vec::new(),
					has_next_page: false,
				});
			}
			return Ok(MangaPageResult {
				entries: Self::fetch_popular_period("DAY")?,
				has_next_page: false,
			});
		}
		match id_ref {
			"latest_updates" => {
				return Self::fetch_static_listing(
					LATEST_UPDATES_QUERY,
					"fetchLatestMangaUpdates",
					None,
					page,
				);
			}
			"latest_titles" => {
				return Self::fetch_static_listing(
					LATEST_TITLES_QUERY,
					"fetchLatestMangaTitles",
					None,
					page,
				);
			}
			_ => {}
		}
		if C::DEFAULT_RATING_INCLUDE.is_empty() {
			match id_ref {
				"top_week" => {
					return Ok(MangaPageResult {
						entries: if page == 1 {
							Self::fetch_popular_period("WEEK")?
						} else {
							Vec::new()
						},
						has_next_page: false,
					});
				}
				"top_month" => {
					return Ok(MangaPageResult {
						entries: if page == 1 {
							Self::fetch_popular_period("MONTH")?
						} else {
							Vec::new()
						},
						has_next_page: false,
					});
				}
				"top_manhwa" => {
					return Self::fetch_static_listing(
						TOP_BY_TYPE_QUERY,
						"fetchTopManhwa",
						Some("MANHWA"),
						page,
					);
				}
				_ => {}
			}
		}
		let type_slug = TYPE_SECTIONS
			.iter()
			.find(|(lid, _, _)| *lid == id_ref)
			.and_then(|(_, _, slug)| *slug);
		Self::fetch_catalog(id_ref, type_slug, page)
	}
}

impl<C: Config> Home for SenkuroEngine<C> {
	fn get_home(&self) -> Result<HomeLayout> {
		if !C::DEFAULT_RATING_INCLUDE.is_empty() {
			return self.get_catalog_home();
		}
		let popular = Self::fetch_popular_period("DAY")
			.or_else(|_| Self::fetch_catalog("popular", None, 1).map(|r| r.entries))?;
		let featured: Vec<Manga> = popular
			.iter()
			.take(HOME_FEATURED_COUNT)
			.cloned()
			.map(|m| self.get_manga_update(m.clone(), true, false).unwrap_or(m))
			.collect();
		let more_popular: Vec<Link> = popular
			.iter()
			.skip(HOME_FEATURED_COUNT)
			.cloned()
			.map(Link::from)
			.collect();
		let weekly = Self::fetch_popular_period("WEEK").unwrap_or_default();
		let monthly = Self::fetch_popular_period("MONTH").unwrap_or_default();
		let latest_updates = Self::fetch_manga_connection(
			LATEST_UPDATES_QUERY,
			"fetchLatestMangaUpdates",
			None,
			HOME_SCROLLER_COUNT,
		)
		.unwrap_or_default();
		let latest_titles = Self::fetch_manga_connection(
			LATEST_TITLES_QUERY,
			"fetchLatestMangaTitles",
			None,
			HOME_SCROLLER_COUNT,
		)
		.unwrap_or_default();
		let top_manhwa = Self::fetch_manga_connection(
			TOP_BY_TYPE_QUERY,
			"fetchTopManhwa",
			Some("MANHWA"),
			HOME_SCROLLER_COUNT,
		)
		.unwrap_or_default();

		let mut components: Vec<HomeComponent> = Vec::with_capacity(6 + TYPE_SECTIONS.len());
		components.push(HomeComponent {
			title: Some("Самое читаемое".to_string()),
			subtitle: section_subtitle("popular"),
			value: HomeComponentValue::BigScroller {
				entries: featured,
				auto_scroll_interval: Some(8.0),
			},
		});
		if !more_popular.is_empty() {
			components.push(HomeComponent {
				title: Some("Ещё за день".to_string()),
				subtitle: section_subtitle("popular"),
				value: HomeComponentValue::Scroller {
					entries: more_popular,
					listing: Some(Listing {
						id: "popular".to_string(),
						name: "Самое читаемое".to_string(),
						kind: ListingKind::Default,
					}),
				},
			});
		}
		push_scroller(&mut components, "top_week", "Читают за неделю", weekly);
		push_scroller(&mut components, "top_month", "Читают за месяц", monthly);
		push_scroller(
			&mut components,
			"latest_updates",
			"Последние обновления",
			latest_updates,
		);
		push_scroller(
			&mut components,
			"latest_titles",
			"Последние манги",
			latest_titles,
		);
		push_scroller(&mut components, "top_manhwa", "Топ манхв", top_manhwa);
		for (lid, title, type_slug) in TYPE_SECTIONS {
			if *lid == "comics" && !C::INCLUDE_COMICS {
				continue;
			}
			let entries = Self::fetch_catalog(*lid, *type_slug, 1)
				.map(|r| r.entries)
				.unwrap_or_default();
			if entries.is_empty() {
				continue;
			}
			let links: Vec<Link> = entries.into_iter().map(Link::from).collect();
			components.push(HomeComponent {
				title: Some((*title).to_string()),
				subtitle: section_subtitle(lid),
				value: HomeComponentValue::Scroller {
					entries: links,
					listing: Some(Listing {
						id: (*lid).to_string(),
						name: (*title).to_string(),
						kind: ListingKind::Default,
					}),
				},
			});
		}
		Ok(HomeLayout { components })
	}
}

fn push_scroller(
	components: &mut Vec<HomeComponent>,
	id: &'static str,
	title: &'static str,
	entries: Vec<Manga>,
) {
	if entries.is_empty() {
		return;
	}
	components.push(HomeComponent {
		title: Some(title.to_string()),
		subtitle: section_subtitle(id),
		value: HomeComponentValue::Scroller {
			entries: entries.into_iter().map(Link::from).collect(),
			listing: Some(Listing {
				id: id.to_string(),
				name: title.to_string(),
				kind: ListingKind::Default,
			}),
		},
	});
}

impl<C: Config> DynamicFilters for SenkuroEngine<C> {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		// Static fixed-enum filters first.
		let mut out = filters::static_filters();

		// Then a multi-select per Senkuro genre root group, populated from the API.
		#[derive(Serialize)]
		struct EmptyVars {}
		let body = serde_json::to_vec(&GqlRequest {
			query: FILTERS_QUERY,
			variables: EmptyVars {},
		})
		.map_err(|e| error!("encode filters: {e}"))?;
		match post_graphql::<C, FiltersResponse>("fetchMangaLabels", &body) {
			Ok(resp) => {
				let labels = resp.all_labels;
				out.extend(filters::dynamic_genre_filters(
					&labels,
					C::EXCLUDE_GENRES,
					!C::DEFAULT_RATING_INCLUDE.is_empty(),
				));
			}
			Err(e) => {
				println!("[senkuro] dynamic filters fetch failed, returning static only: {e:?}");
			}
		}
		Ok(out)
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

fn post_graphql<C: Config, T: DeserializeOwned>(operation: &str, body: &[u8]) -> Result<T> {
	let url = settings::api_url(C::DEFAULT_API_DOMAIN, C::FALLBACK_API_DOMAIN);
	match post_graphql_at::<C, T>(operation, body, &url) {
		Ok(data) => Ok(data),
		Err(primary_error) => {
			let fallback = settings::fallback_api_url(
				&url,
				C::DEFAULT_API_DOMAIN,
				C::FALLBACK_API_DOMAIN,
			);
			println!(
				"[senkuro:{operation}] {url} failed, retrying through {fallback}: {primary_error:?}"
			);
			post_graphql_at::<C, T>(operation, body, &fallback)
		}
	}
}

fn post_graphql_at<C: Config, T: DeserializeOwned>(
	operation: &str,
	body: &[u8],
	url: &str,
) -> Result<T> {
	let response = Request::post(url)?
		.header("Content-Type", "application/json")
		.header("Accept", "application/json")
		.header("App-Id", C::APP_ID)
		.header("Origin", C::BASE_URL)
		.header("Referer", C::BASE_URL)
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
