#![no_std]
extern crate alloc;

use aidoku::prelude::*;
use aidoku::{ImageRequestProvider, ListingProvider, Source, WebLoginHandler};
use grouple::{Config, Grouple};

struct RuReadManga;

impl Config for RuReadManga {
	const NAME: &'static str = "ReadManga";
	const DEFAULT_BASE_URL: &'static str = "https://a.zazaza.me";
}

register_source!(
	Grouple<RuReadManga>,
	ListingProvider,
	ImageRequestProvider,
	WebLoginHandler
);
