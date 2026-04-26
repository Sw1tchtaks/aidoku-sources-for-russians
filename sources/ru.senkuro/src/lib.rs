#![no_std]
extern crate alloc;

use aidoku::prelude::*;
use aidoku::{DeepLinkHandler, DynamicFilters, ImageRequestProvider, Source};
use senkuro::{Config, SenkuroEngine};

struct RuSenkuro;

impl Config for RuSenkuro {
	const SITE: &'static str = "Senkuro";
	const BASE_URL: &'static str = "https://senkuro.com";
	// Senkuro hides 18+ tags by default; mirror the keiyoushi extension.
	const EXCLUDE_GENRES: &'static [&'static str] =
		&["hentai", "yaoi", "yuri", "shoujo_ai", "shounen_ai", "lgbt"];
}

register_source!(
	SenkuroEngine<RuSenkuro>,
	DynamicFilters,
	DeepLinkHandler,
	ImageRequestProvider
);
