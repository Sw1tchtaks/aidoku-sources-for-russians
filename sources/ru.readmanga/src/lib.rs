#![no_std]
extern crate alloc;

use aidoku::prelude::*;
use aidoku::{Home, ImageRequestProvider, ListingProvider, Source, WebLoginHandler};
use grouple::{Config, Grouple};

struct RuReadManga;

impl Config for RuReadManga {
	const NAME: &'static str = "ReadManga";
	const DEFAULT_BASE_URL: &'static str = "https://a.zazaza.me";
	const ENABLE_MANUAL_COOKIES: bool = false;
	const ENABLE_AUTH_TOKEN: bool = false;
}

register_source!(
	Grouple<RuReadManga>,
	ListingProvider,
	Home,
	ImageRequestProvider,
	WebLoginHandler
);
