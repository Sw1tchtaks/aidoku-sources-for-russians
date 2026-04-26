#![no_std]
extern crate alloc;

use aidoku::prelude::*;
use aidoku::{ImageRequestProvider, ListingProvider, Source};
use grouple::{Config, Grouple};

struct RuSelfManga;

impl Config for RuSelfManga {
	const NAME: &'static str = "SelfManga";
	const DEFAULT_BASE_URL: &'static str = "https://1.selfmanga.live";
}

register_source!(Grouple<RuSelfManga>, ListingProvider, ImageRequestProvider);
