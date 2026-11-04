#![no_std]
extern crate alloc;

use aidoku::prelude::*;
use aidoku::{
	DeepLinkHandler, DynamicFilters, Home, ImageRequestProvider, ListingProvider, Source,
};
use senkuro::{Config, SenkuroEngine};

struct RuSenkognito;

impl Config for RuSenkognito {
	const SITE: &'static str = "Senkognito";
	const BASE_URL: &'static str = "https://senkognito.com";
	const DEFAULT_API_DOMAIN: &'static str = "https://api.senkognito.com";
	// Senkuro's public host exposes the same application-scoped data when the
	// Senkognito App-Id is supplied, so it is a useful independent fallback.
	const FALLBACK_API_DOMAIN: &'static str = "https://api.senkuro.com";
	const APP_ID: &'static str = "5033164800100";
	// Senkognito is the adult-content twin of Senkuro; no genre filtering.
	const EXCLUDE_GENRES: &'static [&'static str] = &[];
	// Default to EXPLICIT only — Senkuro's API treats QUESTIONABLE as just another
	// safe tier (returns the same default popular set), so adding it would silently
	// cancel the NSFW filtering. Empirically, only `include: [EXPLICIT]` produces
	// the adult-focused catalog Senkognito users expect.
	const DEFAULT_RATING_INCLUDE: &'static [&'static str] = &["EXPLICIT"];
	// The Senkognito App-Id scopes the API to the adult catalog. Adding a hidden
	// `hentai` label here would incorrectly intersect it with every chosen tag.
	const DEFAULT_LABEL_INCLUDE: &'static [&'static str] = &[];
	const INCLUDE_COMICS: bool = false;
}

register_source!(
	SenkuroEngine<RuSenkognito>,
	ListingProvider,
	Home,
	DynamicFilters,
	DeepLinkHandler,
	ImageRequestProvider
);
