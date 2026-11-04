#![no_std]
extern crate alloc;

mod pages;
mod parser;

use aidoku::helpers::uri::encode_uri_component;
use aidoku::imports::defaults::{DefaultValue, defaults_get, defaults_set};
use aidoku::imports::html::{Document, Html};
use aidoku::imports::net::{Request, TimeUnit, set_rate_limit};
use aidoku::prelude::*;
use aidoku::{
	Chapter, FilterValue, HashMap, Home, HomeComponent, HomeComponentValue, HomeLayout,
	ImageRequestProvider, Link, Listing, ListingKind, ListingProvider, Manga, MangaPageResult,
	Page, PageContent, PageContext, Result, Source, WebLoginHandler,
	alloc::{String, Vec},
};
use alloc::format;
use alloc::string::ToString;
use core::marker::PhantomData;

pub trait Config: 'static {
	/// Public-facing source name (used by error messages only).
	const NAME: &'static str;
	/// Default web base URL with no trailing slash. Users can override via the
	/// `baseUrl` setting.
	const DEFAULT_BASE_URL: &'static str;
	/// User-Agent string. Most Grouple sites accept "arora", but a normal UA
	/// works too. Override per-source if a particular site is finicky.
	const USER_AGENT: &'static str = "arora";
	/// Whether to replay the free-form Cookie setting.
	const ENABLE_MANUAL_COOKIES: bool = true;
	/// Whether to replay the optional Grouple JWT setting.
	const ENABLE_AUTH_TOKEN: bool = true;
}

const PAGE_SIZE: i32 = 50;
const HOME_FEATURED_COUNT: usize = 3;
const HOME_SCROLLER_COUNT: usize = 18;

pub struct Grouple<C: Config>(PhantomData<C>);

impl<C: Config> Default for Grouple<C> {
	fn default() -> Self {
		Self(PhantomData)
	}
}

fn cookie_key<C: Config>() -> String {
	format!("grouple.cookie.{}", C::NAME)
}

fn stored_cookie<C: Config>() -> Option<String> {
	defaults_get::<String>(&cookie_key::<C>()).filter(|s| !s.is_empty())
}

fn store_cookie<C: Config>(value: &str) {
	defaults_set(
		&cookie_key::<C>(),
		DefaultValue::String(value.to_string()),
	);
}

/// Per-source auth token (Grouple lets logged-in users grab a JWT at
/// `https://3.grouple.co/private/settings/token`). When set, replayed in
/// `Authorization: Bearer …` on every request — works alongside the cookie
/// jar so users can pick whichever auth path is more convenient.
fn stored_token<C: Config>() -> Option<String> {
	if C::ENABLE_AUTH_TOKEN {
		defaults_get::<String>("authToken").filter(|s| !s.is_empty())
	} else {
		None
	}
}

/// Free-form Cookie header set by the user from browser DevTools. The most
/// reliable escape hatch: when the WebView login closes too early, the
/// per-Grouple JWT isn't accepted by the HTML pages, and Google OAuth is
/// blocked in WebKit, the user can copy the entire `Cookie:` header from a
/// logged-in browser session and paste it here.
fn manual_cookies<C: Config>() -> Option<String> {
	if C::ENABLE_MANUAL_COOKIES {
		defaults_get::<String>("manualCookies").filter(|s| !s.is_empty())
	} else {
		None
	}
}

impl<C: Config> Grouple<C> {
	fn base_url() -> String {
		let mut url =
			defaults_get::<String>("baseUrl").unwrap_or_else(|| C::DEFAULT_BASE_URL.to_string());
		if url.ends_with('/') {
			url.pop();
		}
		url
	}

	fn build_request(url: &str) -> Result<Request> {
		let base = Self::base_url();
		let mut req = Request::get(url)?
			.header("User-Agent", C::USER_AGENT)
			.header("Referer", &base)
			.header(
				"Accept",
				"text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
			)
			.header("Accept-Language", "ru,en;q=0.9");
		// Cookie precedence: manual paste wins (it's the user's own logged-in
		// session, byte-for-byte), otherwise replay whatever the WebView
		// captured. Fold them together when both are set so cookies set by
		// post-login navigation don't get dropped.
		match (manual_cookies::<C>(), stored_cookie::<C>()) {
			(Some(m), Some(c)) => req = req.header("Cookie", &format!("{m}; {c}")),
			(Some(m), None) => req = req.header("Cookie", &m),
			(None, Some(c)) => req = req.header("Cookie", &c),
			(None, None) => {}
		}
		if let Some(t) = stored_token::<C>() {
			req = req.header("Authorization", &format!("Bearer {t}"));
		}
		Ok(req)
	}

	fn fetch_html(url: &str) -> Result<Document> {
		let base = Self::base_url();
		let response = Self::build_request(url)?.send()?;
		let status = response.status_code();
		let bytes = response.get_data()?;
		if !(200..400).contains(&status) {
			return Err(error!("{} HTTP {} for {}", C::NAME, status, url));
		}
		Html::parse_with_url(bytes, &base).map_err(|e| error!("{} parse error: {:?}", C::NAME, e))
	}

	fn parse_listing(doc: &Document) -> MangaPageResult {
		let base = Self::base_url();
		let entries = doc
			.select("div.tile")
			.map(|list| {
				list.into_iter()
					.filter_map(|el| parser::parse_tile(&el, &base))
					.collect::<Vec<_>>()
			})
			.unwrap_or_default();
		let has_next_page = doc.select_first("a.nextLink").is_some();
		MangaPageResult {
			entries,
			has_next_page,
		}
	}

	fn fetch_listing(sort_type: &str, page: i32) -> Result<MangaPageResult> {
		let base = Self::base_url();
		let offset = PAGE_SIZE * (page - 1).max(0);
		let url = format!("{base}/list?sortType={sort_type}&offset={offset}");
		let doc = Self::fetch_html(&url)?;
		Ok(Self::parse_listing(&doc))
	}
}

impl<C: Config> Source for Grouple<C> {
	fn new() -> Self {
		set_rate_limit(2, 1, TimeUnit::Seconds);
		Self::default()
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		_filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let base = Self::base_url();
		let offset = PAGE_SIZE * (page - 1).max(0);
		let url = if let Some(q) = query.as_ref().filter(|q| !q.trim().is_empty()) {
			format!(
				"{base}/search/advancedResults?offset={offset}&q={}",
				encode_uri_component(q.as_str())
			)
		} else {
			format!("{base}/list?sortType=rate&offset={offset}")
		};
		let doc = Self::fetch_html(&url)?;
		Ok(Self::parse_listing(&doc))
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let key = manga.key.clone();
		let base = Self::base_url();
		let mut updated = manga;

		if needs_details || needs_chapters {
			let url = format!("{base}{key}");
			let doc = Self::fetch_html(&url)?;

			if needs_details {
				updated = parser::parse_details(&doc, &base, &key, updated);
			}

			if needs_chapters {
				let query = parser::extract_chapter_query(&doc);
				let chapters: Vec<Chapter> = doc
					.select("tr.item-row:has(td > a):has(td.date:not(.text-info))")
					.map(|list| {
						list.into_iter()
							.filter_map(|el| parser::parse_chapter_row(&el, &key, &query, &base))
							.collect::<Vec<_>>()
					})
					.unwrap_or_default();
				updated.chapters = Some(chapters);
			}
		}

		Ok(updated)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let base = Self::base_url();
		let url = format!("{base}{}", chapter.key);
		let response = Self::build_request(&url)?.send()?;
		let status = response.status_code();
		let bytes = response.get_data()?;
		if !(200..400).contains(&status) {
			return Err(error!("{} HTTP {} for {}", C::NAME, status, url));
		}
		let html = String::from_utf8_lossy(&bytes).into_owned();
		let urls = pages::extract_pages(&html, &base);
		if urls.is_empty() {
			println!(
				"[{}] no pages parsed for {} (possible reader format change)",
				C::NAME,
				chapter.key
			);
		}
		Ok(urls
			.into_iter()
			.map(|u| Page {
				content: PageContent::url(u),
				..Default::default()
			})
			.collect())
	}
}

impl<C: Config> ListingProvider for Grouple<C> {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let sort_type = match listing.id.as_str() {
			"latest" => "updated",
			"new" => "created",
			_ => "rate",
		};
		Self::fetch_listing(sort_type, page)
	}
}

impl<C: Config> Home for Grouple<C> {
	fn get_home(&self) -> Result<HomeLayout> {
		let popular = Self::fetch_listing("rate", 1)?.entries;
		let latest = Self::fetch_listing("updated", 1)
			.map(|r| r.entries)
			.unwrap_or_default();
		let new = Self::fetch_listing("created", 1)
			.map(|r| r.entries)
			.unwrap_or_default();

		let featured: Vec<Manga> = popular
			.iter()
			.take(HOME_FEATURED_COUNT)
			.cloned()
			.map(|m| self.get_manga_update(m.clone(), true, false).unwrap_or(m))
			.collect();
		let more_popular: Vec<Manga> = popular
			.into_iter()
			.skip(HOME_FEATURED_COUNT)
			.take(HOME_SCROLLER_COUNT)
			.collect();

		let mut components = Vec::with_capacity(4);
		if !featured.is_empty() {
			components.push(HomeComponent {
				title: Some("Популярное".to_string()),
				subtitle: Some("Лучшие тайтлы".to_string()),
				value: HomeComponentValue::BigScroller {
					entries: featured,
					auto_scroll_interval: Some(8.0),
				},
			});
		}
		push_home_scroller(
			&mut components,
			"popular",
			"Ещё популярное",
			"Продолжение подборки",
			more_popular,
		);
		push_home_scroller(
			&mut components,
			"latest",
			"Последние обновления",
			"Свежие главы",
			latest,
		);
		push_home_scroller(&mut components, "new", "Новинки", "Новые тайтлы", new);

		Ok(HomeLayout { components })
	}
}

fn push_home_scroller(
	components: &mut Vec<HomeComponent>,
	id: &'static str,
	title: &'static str,
	subtitle: &'static str,
	entries: Vec<Manga>,
) {
	if entries.is_empty() {
		return;
	}
	components.push(HomeComponent {
		title: Some(title.to_string()),
		subtitle: Some(subtitle.to_string()),
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

impl<C: Config> ImageRequestProvider for Grouple<C> {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		let base = Self::base_url();
		let mut req = Request::get(url)?
			.header("User-Agent", "Mozilla/5.0 (Windows NT 6.3; WOW64)")
			.header("Referer", &base);
		// Cookie precedence: manual paste wins (it's the user's own logged-in
		// session, byte-for-byte), otherwise replay whatever the WebView
		// captured. Fold them together when both are set so cookies set by
		// post-login navigation don't get dropped.
		match (manual_cookies::<C>(), stored_cookie::<C>()) {
			(Some(m), Some(c)) => req = req.header("Cookie", &format!("{m}; {c}")),
			(Some(m), None) => req = req.header("Cookie", &m),
			(None, Some(c)) => req = req.header("Cookie", &c),
			(None, None) => {}
		}
		if let Some(t) = stored_token::<C>() {
			req = req.header("Authorization", &format!("Bearer {t}"));
		}
		Ok(req)
	}
}

impl<C: Config> WebLoginHandler for Grouple<C> {
	fn handle_web_login(&self, _key: String, cookies: HashMap<String, String>) -> Result<bool> {
		// Aidoku invokes this every time the WebView's cookie jar changes.
		// Grouple sets ~3 cookies on initial page load (JSESSIONID, CSRF,
		// site prefs) BEFORE the user has typed anything — if we returned
		// `true` for that set, Aidoku would close the WebView immediately
		// and the user never gets to log in. Return `false` until we see
		// a marker that only appears after successful authentication.
		if cookies.is_empty() {
			return Ok(false);
		}

		let has_login_marker = cookies.iter().any(|(name, value)| {
			let lower = name.to_lowercase();
			lower.contains("auth")
				|| lower.contains("token")
				|| lower.contains("logged")
				|| lower.contains("user_id")
				|| lower == "remember_me"
				|| lower.starts_with("user_")
				|| lower.starts_with("lab_session")
				// JWT-formatted value (Grouple's user_session sometimes carries one)
				|| value.starts_with("eyJ")
		});

		// Belt-and-braces: a logged-in Grouple jar usually has 5+ cookies
		// (JSESSIONID + remember_me + at least one user/role hint).
		if !has_login_marker && cookies.len() < 5 {
			println!(
				"[{}] login: {} cookies, no auth marker yet — keeping WebView open",
				C::NAME,
				cookies.len()
			);
			return Ok(false);
		}

		let mut header = String::new();
		for (k, v) in cookies.iter() {
			if !header.is_empty() {
				header.push_str("; ");
			}
			header.push_str(k);
			header.push('=');
			header.push_str(v);
		}
		store_cookie::<C>(&header);
		println!(
			"[{}] login: stored {} cookies (marker={})",
			C::NAME,
			cookies.len(),
			has_login_marker
		);
		Ok(true)
	}
}
