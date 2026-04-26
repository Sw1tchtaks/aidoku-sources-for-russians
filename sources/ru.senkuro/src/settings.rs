use aidoku::imports::defaults::defaults_get;
use alloc::string::{String, ToString};

const DOMAIN_KEY: &str = "domain";
const ENGLISH_TITLES_KEY: &str = "englishTitles";
const ORIGINAL_QUALITY_KEY: &str = "originalQuality";

pub fn domain() -> String {
	defaults_get::<String>(DOMAIN_KEY).unwrap_or_else(|| "senkuro.com".to_string())
}

pub fn prefer_english_titles() -> bool {
	defaults_get::<bool>(ENGLISH_TITLES_KEY).unwrap_or(false)
}

pub fn prefer_original_quality() -> bool {
	defaults_get::<bool>(ORIGINAL_QUALITY_KEY).unwrap_or(false)
}
