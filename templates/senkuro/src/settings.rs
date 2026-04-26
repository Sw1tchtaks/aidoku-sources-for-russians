use aidoku::imports::defaults::defaults_get;
use alloc::string::{String, ToString};

const API_DOMAIN_KEY: &str = "apiDomain";
const ENGLISH_TITLES_KEY: &str = "englishTitles";

pub const DEFAULT_API_DOMAIN: &str = "https://api.senkuro.me";

pub fn api_url() -> String {
	let mut base =
		defaults_get::<String>(API_DOMAIN_KEY).unwrap_or_else(|| DEFAULT_API_DOMAIN.to_string());
	if base.ends_with('/') {
		base.pop();
	}
	base.push_str("/graphql");
	base
}

pub fn prefer_english_titles() -> bool {
	defaults_get::<bool>(ENGLISH_TITLES_KEY).unwrap_or(false)
}
