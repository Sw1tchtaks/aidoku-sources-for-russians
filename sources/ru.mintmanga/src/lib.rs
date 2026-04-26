#![no_std]
extern crate alloc;

use aidoku::prelude::*;
use aidoku::{ImageRequestProvider, ListingProvider, Source};
use grouple::{Config, Grouple};

struct RuMintManga;

impl Config for RuMintManga {
	const NAME: &'static str = "MintManga";
	const DEFAULT_BASE_URL: &'static str = "https://2.mintmanga.one";
}

register_source!(Grouple<RuMintManga>, ListingProvider, ImageRequestProvider);
