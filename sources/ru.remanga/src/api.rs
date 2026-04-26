use aidoku::imports::defaults::{DefaultValue, defaults_get, defaults_set};
use aidoku::imports::net::Request;
use aidoku::prelude::*;
use aidoku::{Result, alloc::String, error};
use alloc::format;
use alloc::string::ToString;
use serde::de::DeserializeOwned;

pub const API_URL: &str = "https://api.remanga.org";
pub const SITE_URL: &str = "https://remanga.org";

const TOKEN_KEY: &str = "authToken";

pub fn token() -> Option<String> {
	defaults_get::<String>(TOKEN_KEY).filter(|s| !s.is_empty())
}

pub fn store_token(value: &str) {
	defaults_set(TOKEN_KEY, DefaultValue::String(value.to_string()));
}

pub fn apply_headers(request: Request) -> Request {
	let mut req = request
		.header(
			"User-Agent",
			"Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
		)
		.header("Referer", SITE_URL)
		.header("Origin", SITE_URL)
		.header("Accept", "application/json")
		.header("Accept-Language", "ru,en;q=0.9");
	if let Some(t) = token() {
		req = req.header("Authorization", &format!("bearer {t}"));
	}
	req
}

pub fn get_json<T: DeserializeOwned>(path: &str) -> Result<T> {
	let url = if path.starts_with("http") {
		path.to_string()
	} else {
		format!("{API_URL}{path}")
	};
	let request = apply_headers(Request::get(&url)?);
	let response = request.send()?;
	let status = response.status_code();
	let bytes = response.get_data()?;
	if !(200..300).contains(&status) {
		let preview = preview(&bytes);
		println!("[remanga] HTTP {status} for {url}: {preview}");
		return Err(error!("Remanga HTTP {status}: {preview}"));
	}
	serde_json::from_slice(&bytes).map_err(|e| {
		let preview = preview(&bytes);
		error!("Remanga decode error: {e}; body: {preview}")
	})
}

fn preview(bytes: &[u8]) -> String {
	let n = bytes.len().min(300);
	String::from_utf8_lossy(&bytes[..n]).into_owned()
}
