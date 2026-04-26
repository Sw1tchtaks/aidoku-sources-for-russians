#![no_std]
extern crate alloc;

use aidoku::prelude::*;
use aidoku::{DeepLinkHandler, DynamicFilters, ImageRequestProvider, Source};
use senkuro::{Config, SenkuroEngine};

struct RuSenkognito;

impl Config for RuSenkognito {
	const SITE: &'static str = "Senkognito";
	const BASE_URL: &'static str = "https://senkognito.com";
	// Senkognito is the adult-content twin of Senkuro; no genre filtering.
	const EXCLUDE_GENRES: &'static [&'static str] = &[];
	// Default to EXPLICIT+QUESTIONABLE so the catalog actually shows adult content
	// the site is built for. Without this, the API returns the same safe-by-default
	// set as Senkuro.
	const DEFAULT_RATING_INCLUDE: &'static [&'static str] = &["EXPLICIT", "QUESTIONABLE"];
}

register_source!(
	SenkuroEngine<RuSenkognito>,
	DynamicFilters,
	DeepLinkHandler,
	ImageRequestProvider
);
