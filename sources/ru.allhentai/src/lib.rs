#![no_std]
extern crate alloc;

use aidoku::prelude::*;
use aidoku::{ImageRequestProvider, ListingProvider, Source, WebLoginHandler};
use grouple::{Config, Grouple};

struct RuAllHentai;

impl Config for RuAllHentai {
	const NAME: &'static str = "AllHentai";
	const DEFAULT_BASE_URL: &'static str = "https://20.allhen.online";
}

register_source!(
	Grouple<RuAllHentai>,
	ListingProvider,
	ImageRequestProvider,
	WebLoginHandler
);
