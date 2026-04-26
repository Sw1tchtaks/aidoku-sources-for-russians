use crate::models::{
	ChapterPagesData, ChaptersData, GqlResponse, MangaData, MangaNode, SearchData, SearchNode,
};
use crate::settings::domain;
use aidoku::imports::net::Request;
use aidoku::prelude::println;
use aidoku::{Result, alloc::String, error};
use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;
use serde::de::DeserializeOwned;

const API_URL: &str = "https://api.senkuro.me/graphql";

const SEARCH_HASH: &str = "e64937b4fc9c921c2141f2995473161bed921c75855c5de934752392175936bc";
const FETCH_MANGA_HASH: &str = "6d8b28abb9a9ee3199f6553d8f0a61c005da8f5c56a88ebcf3778eff28d45bd5";
const FETCH_CHAPTERS_HASH: &str = "8c854e121f05aa93b0c37889e732410df9ea207b4186c965c845a8d970bdcc12";
const FETCH_CHAPTER_HASH: &str = "320a2637126c71ccdbce6af04325fe2f5878cc7adf9e90d06bdd6752f9bbb14e";

pub fn base_url() -> String {
	format!("https://{}", domain())
}

pub fn apply_headers(request: Request) -> Request {
	let base = base_url();
	request
		.header(
			"User-Agent",
			"Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
		)
		.header("Referer", base.as_str())
		.header("Origin", base.as_str())
		.header("Accept", "*/*")
		.header("Accept-Language", "ru,en;q=0.9")
}

fn post_graphql<T: DeserializeOwned>(operation: &str, body: &str) -> Result<T> {
	let request = apply_headers(
		Request::post(API_URL)?
			.header("Content-Type", "application/json")
			.body(body.as_bytes()),
	);

	let response = request.send()?;
	let status = response.status_code();
	let bytes = response.get_data()?;

	if status < 200 || status >= 300 {
		let preview = preview_body(&bytes);
		println!("[senkuro:{operation}] HTTP {status}: {preview}");
		return Err(error!("Senkuro {operation} HTTP {status}"));
	}

	let raw: GqlResponse<T> = match serde_json::from_slice(&bytes) {
		Ok(v) => v,
		Err(e) => {
			let preview = preview_body(&bytes);
			println!("[senkuro:{operation}] JSON parse error: {e}, body: {preview}");
			return Err(error!("Senkuro {operation} parse error: {e}"));
		}
	};

	if let Some(errors) = raw.errors {
		let messages: Vec<String> = errors.into_iter().map(|e| e.message).collect();
		let joined = messages.join("; ");
		println!("[senkuro:{operation}] GraphQL errors: {joined}");
		return Err(error!("Senkuro {operation}: {joined}"));
	}

	raw.data.ok_or_else(|| {
		let preview = preview_body(&bytes);
		println!("[senkuro:{operation}] empty data, body: {preview}");
		error!("Senkuro {operation}: empty data")
	})
}

fn preview_body(bytes: &[u8]) -> String {
	let limit = bytes.len().min(400);
	String::from_utf8_lossy(&bytes[..limit]).into_owned()
}

fn escape_json(input: &str) -> String {
	let mut out = String::with_capacity(input.len() + 2);
	for c in input.chars() {
		match c {
			'"' => out.push_str("\\\""),
			'\\' => out.push_str("\\\\"),
			'\n' => out.push_str("\\n"),
			'\r' => out.push_str("\\r"),
			'\t' => out.push_str("\\t"),
			c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
			c => out.push(c),
		}
	}
	out
}

pub fn search_mangas(query: &str) -> Result<Vec<SearchNode>> {
	let body = format!(
		r#"{{"operationName":"search","variables":{{"query":"{q}","type":"MANGA"}},"extensions":{{"persistedQuery":{{"version":1,"sha256Hash":"{hash}"}}}}}}"#,
		q = escape_json(query),
		hash = SEARCH_HASH,
	);
	let data: SearchData = post_graphql("search", &body)?;
	let nodes: Vec<SearchNode> = data
		.search
		.map(|c| c.edges.into_iter().filter_map(|e| e.node).collect())
		.unwrap_or_default();
	println!("[senkuro:search] query={query:?} -> {} results", nodes.len());
	Ok(nodes)
}

pub fn fetch_manga(slug: &str) -> Result<MangaNode> {
	let body = format!(
		r#"{{"operationName":"fetchManga","variables":{{"slug":"{slug}"}},"extensions":{{"persistedQuery":{{"version":1,"sha256Hash":"{hash}"}}}}}}"#,
		slug = escape_json(slug),
		hash = FETCH_MANGA_HASH,
	);
	let data: MangaData = post_graphql("fetchManga", &body)?;
	data.manga
		.ok_or_else(|| error!("Manga \"{}\" not found", slug))
}

pub fn fetch_manga_chapters(branch_id: &str, after: Option<&str>) -> Result<ChaptersData> {
	let after_part = match after {
		Some(c) => format!(r#""{}""#, escape_json(c)),
		None => "null".to_string(),
	};
	let body = format!(
		r#"{{"operationName":"fetchMangaChapters","variables":{{"after":{after_part},"branchId":"{branch}","number":null,"orderBy":{{"direction":"DESC","field":"NUMBER"}}}},"extensions":{{"persistedQuery":{{"version":1,"sha256Hash":"{hash}"}}}}}}"#,
		branch = escape_json(branch_id),
		hash = FETCH_CHAPTERS_HASH,
	);
	post_graphql("fetchMangaChapters", &body)
}

pub fn fetch_chapter_pages(slug: &str) -> Result<ChapterPagesData> {
	let body = format!(
		r#"{{"operationName":"fetchMangaChapter","variables":{{"slug":"{slug}","cdnQuality":"auto"}},"extensions":{{"persistedQuery":{{"version":1,"sha256Hash":"{hash}"}}}}}}"#,
		slug = escape_json(slug),
		hash = FETCH_CHAPTER_HASH,
	);
	post_graphql("fetchMangaChapter", &body)
}
