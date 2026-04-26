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
	// Senkognito is the adult-content twin of Senkuro; no genre filtering.
	const EXCLUDE_GENRES: &'static [&'static str] = &[];
	// Default to EXPLICIT only — Senkuro's API treats QUESTIONABLE as just another
	// safe tier (returns the same default popular set), so adding it would silently
	// cancel the NSFW filtering. Empirically, only `include: [EXPLICIT]` produces
	// the adult-focused catalog Senkognito users expect.
	const DEFAULT_RATING_INCLUDE: &'static [&'static str] = &["EXPLICIT"];
}

register_source!(
	SenkuroEngine<RuSenkognito>,
	ListingProvider,
	Home,
	DynamicFilters,
	DeepLinkHandler,
	ImageRequestProvider
);
