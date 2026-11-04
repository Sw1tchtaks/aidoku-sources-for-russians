use alloc::string::String;
use alloc::vec::Vec;
use serde::Serialize;

// GraphQL query bodies. Names match Tachiyomi-specific operations exposed by Senkuro's
// public schema. Unlike Apollo persisted queries (which break when the frontend updates
// its hash registry), these are stable as long as the GraphQL schema doesn't break.

pub const MANGAS_QUERY: &str = r#"query searchTachiyomiManga($query: String, $type: MangaTachiyomiSearchTypeFilter, $status: MangaTachiyomiSearchStatusFilter, $translationStatus: MangaTachiyomiSearchTranslationStatusFilter, $label: MangaTachiyomiSearchLabelFilter, $format: MangaTachiyomiSearchFormatFilter, $rating: MangaTachiyomiSearchRatingFilter, $offset: Int) { mangaTachiyomiSearch(query: $query, type: $type, status: $status, translationStatus: $translationStatus, label: $label, format: $format, rating: $rating, offset: $offset) { mangas { id slug originalName { lang content } titles { lang content } alternativeNames { lang content } cover { original { url } preview: resize(width: 350, height: 500) { url } } } } }"#;

pub const WEB_MANGAS_QUERY: &str = r#"query fetchMangas($first: Int!, $after: String, $search: String, $type: MangaTypeFilter, $format: MangaFormatFilter, $status: MangaStatusFilter, $source: MangaSourceFilter, $rating: MangaRatingFilter, $chapters: MangaChaptersFilter, $originCountry: MangaOriginCountryFilter, $releasedOn: MangaReleasedOnFilter, $label: MangaLabelFilter, $translitionStatus: MangaTranslitionStatusFilter, $orderField: MangaOrderField!, $orderDirection: OrderDirection!) { mangas(first: $first, after: $after, orderBy: { field: $orderField, direction: $orderDirection }, search: $search, type: $type, format: $format, status: $status, source: $source, rating: $rating, chapters: $chapters, originCountry: $originCountry, releasedOn: $releasedOn, label: $label, translitionStatus: $translitionStatus) { edges { node { id slug originalName { lang content } titles { lang content } alternativeNames { lang content } type rating formats score cover { original { url } preview: resize(width: 350, height: 500) { url } } } } pageInfo { hasNextPage endCursor } } }"#;

pub const DETAILS_QUERY: &str = r#"query fetchTachiyomiManga($mangaId: ID!) { mangaTachiyomiInfo(mangaId: $mangaId) { id slug originalName { lang content } titles { lang content } alternativeNames { lang content } localizations { lang description } type rating score status formats labels { id rootId slug titles { lang content } } translationStatus cover { original { url } preview: resize(width: 350, height: 500) { url } } mainStaff { roles person { name } } } }"#;

pub const POPULAR_BY_PERIOD_QUERY: &str = r#"query fetchMangaPopularByPeriod($period: MangaPopularPeriod!) { mangaPopularByPeriod(period: $period) { id slug originalName { lang content } titles { lang content } alternativeNames { lang content } type rating formats score cover { original { url } preview: resize(width: 350, height: 500) { url } } } }"#;

pub const LATEST_UPDATES_QUERY: &str = r#"query fetchLatestMangaUpdates($first: Int!, $type: MangaTypeFilter, $label: MangaLabelFilter, $rating: MangaRatingFilter) { mangas(first: $first, orderBy: { field: LAST_CHAPTER_AT, direction: DESC }, chapters: { start: 1 }, type: $type, label: $label, rating: $rating) { edges { node { id slug originalName { lang content } titles { lang content } alternativeNames { lang content } type rating formats score cover { original { url } preview: resize(width: 350, height: 500) { url } } lastChapters { id slug number volume name createdAt } } } } }"#;

pub const LATEST_TITLES_QUERY: &str = r#"query fetchLatestMangaTitles($first: Int!, $type: MangaTypeFilter, $label: MangaLabelFilter, $rating: MangaRatingFilter) { mangas(first: $first, orderBy: { field: CREATED_AT, direction: DESC }, type: $type, label: $label, rating: $rating) { edges { node { id slug originalName { lang content } titles { lang content } alternativeNames { lang content } type rating formats score cover { original { url } preview: resize(width: 350, height: 500) { url } } } } } }"#;

pub const TOP_BY_TYPE_QUERY: &str = r#"query fetchTopMangaByType($first: Int!, $type: MangaTypeFilter, $label: MangaLabelFilter, $rating: MangaRatingFilter) { mangas(first: $first, orderBy: { field: SCORE, direction: DESC }, type: $type, label: $label, rating: $rating) { edges { node { id slug originalName { lang content } titles { lang content } alternativeNames { lang content } type rating formats score cover { original { url } preview: resize(width: 350, height: 500) { url } } } } } }"#;

pub const CHAPTERS_QUERY: &str = r#"query fetchTachiyomiChapters($mangaId: ID!) { mangaTachiyomiChapters(mangaId: $mangaId) { message chapters { id slug branchId name teamIds number volume createdAt } teams { id slug name } } }"#;

pub const PAGES_QUERY: &str = r#"query fetchTachiyomiChapterPages($mangaId: ID!, $chapterId: ID!) { mangaTachiyomiChapterPages(mangaId: $mangaId, chapterId: $chapterId) { pages { url } } }"#;

pub const FILTERS_QUERY: &str = r#"query fetchMangaLabels { allLabels(subjectType: MANGA) { id rootId slug depth rating titles { lang content } } }"#;

pub const PAGE_SIZE: i32 = 10;

#[derive(Serialize)]
pub struct GqlRequest<'a, V: Serialize> {
	pub query: &'a str,
	pub variables: V,
}

#[derive(Serialize)]
pub struct MangasVariables {
	#[serde(rename = "query", skip_serializing_if = "Option::is_none")]
	pub search: Option<String>,
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
pub struct PeriodVariables<'a> {
	pub period: &'a str,
}

#[derive(Serialize)]
pub struct MangaConnectionVariables {
	pub first: i32,
	#[serde(rename = "type", skip_serializing_if = "Option::is_none")]
	pub kind: Option<FiltersDto>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub label: Option<FiltersDto>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub rating: Option<FiltersDto>,
}

#[derive(Serialize)]
pub struct MangaCatalogVariables {
	pub first: i32,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub after: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub search: Option<String>,
	#[serde(rename = "type", skip_serializing_if = "Option::is_none")]
	pub kind: Option<FiltersDto>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub format: Option<FiltersDto>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub status: Option<FiltersDto>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub source: Option<FiltersDto>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub rating: Option<FiltersDto>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub chapters: Option<NumberRangeDto>,
	#[serde(rename = "originCountry", skip_serializing_if = "Option::is_none")]
	pub origin_country: Option<FiltersDto>,
	#[serde(rename = "releasedOn", skip_serializing_if = "Option::is_none")]
	pub released_on: Option<DateRangeDto>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub label: Option<FiltersDto>,
	#[serde(rename = "translitionStatus", skip_serializing_if = "Option::is_none")]
	pub translation_status: Option<FiltersDto>,
	#[serde(rename = "orderField")]
	pub order_field: &'static str,
	#[serde(rename = "orderDirection")]
	pub order_direction: &'static str,
}

#[derive(Serialize)]
pub struct NumberRangeDto {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub start: Option<i32>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub end: Option<i32>,
}

#[derive(Serialize)]
pub struct DateRangeDto {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub start: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub end: Option<String>,
}

#[derive(Serialize)]
pub struct PagesVariables<'a> {
	#[serde(rename = "mangaId")]
	pub manga_id: &'a str,
	#[serde(rename = "chapterId")]
	pub chapter_id: &'a str,
}
