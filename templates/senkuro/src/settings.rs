use aidoku::imports::defaults::defaults_get;
use alloc::string::{String, ToString};

const API_DOMAIN_KEY: &str = "apiDomain";
const ENGLISH_TITLES_KEY: &str = "englishTitles";

pub fn fallback_api_url(
	current: &str,
	default_domain: &str,
	fallback_domain: &str,
) -> String {
	let base = if current.starts_with(default_domain) {
		fallback_domain
	} else {
		default_domain
	};
	alloc::format!("{base}/graphql")
}

pub fn api_url(default_domain: &str, fallback_domain: &str) -> String {
	let mut base = defaults_get::<String>(API_DOMAIN_KEY)
		.filter(|value| {
			let value = value.trim_end_matches('/');
			value == default_domain || value == fallback_domain
		})
		.unwrap_or_else(|| default_domain.to_string());
	if base.ends_with('/') {
		base.pop();
	}
	base.push_str("/graphql");
	base
}

pub fn prefer_english_titles() -> bool {
	defaults_get::<bool>(ENGLISH_TITLES_KEY).unwrap_or(false)
}
