#![no_std]
extern crate alloc;

use aidoku::prelude::*;
use aidoku::{DeepLinkHandler, ImageRequestProvider, Source};
use senkuro::{Config, SenkuroEngine};

struct RuSenkognito;

impl Config for RuSenkognito {
	const SITE: &'static str = "Senkognito";
	const BASE_URL: &'static str = "https://senkognito.com";
	// Senkognito is the adult-content twin of Senkuro; no genre filtering.
	const EXCLUDE_GENRES: &'static [&'static str] = &[];
}

register_source!(SenkuroEngine<RuSenkognito>, DeepLinkHandler, ImageRequestProvider);
