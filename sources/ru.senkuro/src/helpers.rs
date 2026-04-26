use crate::models::{
	ChapterPagesData, ChaptersData, GqlResponse, MangaData, MangaNode, SearchData, SearchNode,
};
use crate::settings::domain;
use aidoku::imports::net::Request;
use aidoku::{Result, alloc::String, error};
use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;
use serde::de::DeserializeOwned;

pub const PAGE_SIZE: usize = 30;

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
		.header("User-Agent", "Aidoku/0.7 (Senkuro Source)")
		.header("Referer", base.as_str())
		.header("Origin", base.as_str())
		.header("Accept", "application/json")
}

fn post_graphql<T: DeserializeOwned>(body: &str) -> Result<T> {
	let raw: GqlResponse<T> = apply_headers(
		Request::post(API_URL)?
			.header("Content-Type", "application/json")
			.body(body.as_bytes()),
	)
	.json_owned::<GqlResponse<T>>()?;

	if let Some(errors) = raw.errors {
		if let Some(first) = errors.into_iter().next() {
			return Err(error!("Senkuro API error: {}", first.message));
		}
	}

	raw.data.ok_or_else(|| error!("Senkuro API: empty data"))
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
	let data: SearchData = post_graphql(&body)?;
	Ok(data
		.search
		.map(|c| c.edges.into_iter().filter_map(|e| e.node).collect())
		.unwrap_or_default())
}

pub fn fetch_manga(slug: &str) -> Result<MangaNode> {
	let body = format!(
		r#"{{"operationName":"fetchManga","variables":{{"slug":"{slug}"}},"extensions":{{"persistedQuery":{{"version":1,"sha256Hash":"{hash}"}}}}}}"#,
		slug = escape_json(slug),
		hash = FETCH_MANGA_HASH,
	);
	let data: MangaData = post_graphql(&body)?;
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
	post_graphql(&body)
}

pub fn fetch_chapter_pages(slug: &str) -> Result<ChapterPagesData> {
	let body = format!(
		r#"{{"operationName":"fetchMangaChapter","variables":{{"slug":"{slug}","cdnQuality":"auto"}},"extensions":{{"persistedQuery":{{"version":1,"sha256Hash":"{hash}"}}}}}}"#,
		slug = escape_json(slug),
		hash = FETCH_CHAPTER_HASH,
	);
	post_graphql(&body)
}
