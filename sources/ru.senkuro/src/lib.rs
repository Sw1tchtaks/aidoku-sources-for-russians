#![no_std]
extern crate alloc;

use aidoku::prelude::*;
use aidoku::{
	DeepLinkHandler, DynamicFilters, Home, ImageRequestProvider, ListingProvider, Source,
};
use senkuro::{Config, SenkuroEngine};

struct RuSenkuro;

impl Config for RuSenkuro {
	const SITE: &'static str = "Senkuro";
	const BASE_URL: &'static str = "https://senkuro.com";
	// Hide every currently active EXPLICIT child label outside Senkognito.
	const EXCLUDE_GENRES: &'static [&'static str] = &[
		"erotica",
		"hentai",
		"yaoi",
		"yuri",
		"shoujo_ai",
		"shounen_ai",
	];
}

register_source!(
	SenkuroEngine<RuSenkuro>,
	ListingProvider,
	Home,
	DynamicFilters,
	DeepLinkHandler,
	ImageRequestProvider
);
