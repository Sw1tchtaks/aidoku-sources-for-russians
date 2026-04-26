use alloc::string::String;
use alloc::vec::Vec;
use serde::Serialize;

// GraphQL query bodies. Names match Tachiyomi-specific operations exposed by Senkuro's
// public schema. Unlike Apollo persisted queries (which break when the frontend updates
// its hash registry), these are stable as long as the GraphQL schema doesn't break.

pub const SEARCH_QUERY: &str = r#"query searchTachiyomiManga($query: String, $type: MangaTachiyomiSearchTypeFilter, $status: MangaTachiyomiSearchStatusFilter, $translationStatus: MangaTachiyomiSearchTranslationStatusFilter, $label: MangaTachiyomiSearchGenreFilter, $format: MangaTachiyomiSearchGenreFilter, $rating: MangaTachiyomiSearchTagFilter, $offset: Int) { mangaTachiyomiSearch(query: $query, type: $type, status: $status, translationStatus: $translationStatus, label: $label, format: $format, rating: $rating, offset: $offset) { mangas { id slug originalName { lang content } titles { lang content } alternativeNames { lang content } cover { original { url } } } } }"#;

pub const DETAILS_QUERY: &str = r#"query fetchTachiyomiManga($mangaId: ID!) { mangaTachiyomiInfo(mangaId: $mangaId) { id slug originalName { lang content } titles { lang content } alternativeNames { lang content } localizations { lang description } type rating status formats labels { id rootId slug titles { lang content } } translationStatus cover { original { url } } mainStaff { roles person { name } } } }"#;

pub const CHAPTERS_QUERY: &str = r#"query fetchTachiyomiChapters($mangaId: ID!) { mangaTachiyomiChapters(mangaId: $mangaId) { message chapters { id slug branchId name teamIds number volume createdAt } teams { id slug name } } }"#;

pub const PAGES_QUERY: &str = r#"query fetchTachiyomiChapterPages($mangaId: ID!, $chapterId: ID!) { mangaTachiyomiChapterPages(mangaId: $mangaId, chapterId: $chapterId) { pages { url } } }"#;

pub const OFFSET_COUNT: i32 = 20;

#[derive(Serialize)]
pub struct GqlRequest<'a, V: Serialize> {
	pub query: &'a str,
	pub variables: V,
}

#[derive(Serialize, Default)]
pub struct SearchVariables {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub query: Option<String>,
	#[serde(rename = "type", skip_serializing_if = "Option::is_none")]
	pub kind: Option<FiltersDto>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub status: Option<FiltersDto>,
	#[serde(rename = "translationStatus", skip_serializing_if = "Option::is_none")]
	pub translation_status: Option<FiltersDto>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub label: Option<FiltersDto>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub format: Option<FiltersDto>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub rating: Option<FiltersDto>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub offset: Option<i32>,
}

#[derive(Serialize, Default, Clone)]
pub struct FiltersDto {
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub include: Vec<String>,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub exclude: Vec<String>,
}

impl FiltersDto {
	pub fn is_empty(&self) -> bool {
		self.include.is_empty() && self.exclude.is_empty()
	}

	pub fn into_option(self) -> Option<Self> {
		if self.is_empty() { None } else { Some(self) }
	}
}

#[derive(Serialize)]
pub struct DetailsVariables<'a> {
	#[serde(rename = "mangaId")]
	pub manga_id: &'a str,
}

#[derive(Serialize)]
pub struct PagesVariables<'a> {
	#[serde(rename = "mangaId")]
	pub manga_id: &'a str,
	#[serde(rename = "chapterId")]
	pub chapter_id: &'a str,
}
