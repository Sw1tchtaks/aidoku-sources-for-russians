use crate::models::LabelDto;
use aidoku::{Filter, MultiSelectFilter};
use alloc::borrow::Cow;
use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;

/// Static fixed-enum filters that don't need API discovery (type/format/status/etc.).
/// Returned at the head of the filter list so they appear first in Aidoku's UI.
pub fn static_filters() -> Vec<Filter> {
	let mut out: Vec<Filter> = Vec::with_capacity(5);
	out.push(
		MultiSelectFilter {
			id: Cow::Borrowed("type"),
			title: Some(Cow::Borrowed("Тип")),
			can_exclude: true,
			uses_tag_style: true,
			options: borrowed(&[
				"Манга", "Манхва", "Маньхуа", "Комикс", "OEL-манга", "РуМанга",
			]),
			ids: Some(borrowed(&[
				"MANGA",
				"MANHWA",
				"MANHUA",
				"COMICS",
				"OEL_MANGA",
				"RU_MANGA",
			])),
			..Default::default()
		}
		.into(),
	);
	out.push(
		MultiSelectFilter {
			id: Cow::Borrowed("format"),
			title: Some(Cow::Borrowed("Формат")),
			can_exclude: true,
			uses_tag_style: true,
			options: borrowed(&[
				"Сборник", "Додзинси", "В цвете", "Сингл", "Веб", "Вебтун", "Ёнкома", "Короткое",
			]),
			ids: Some(borrowed(&[
				"DIGEST",
				"DOUJINSHI",
				"IN_COLOR",
				"SINGLE",
				"WEB",
				"WEBTOON",
				"YONKOMA",
				"SHORT",
			])),
			..Default::default()
		}
		.into(),
	);
	out.push(
		MultiSelectFilter {
			id: Cow::Borrowed("status"),
			title: Some(Cow::Borrowed("Статус выпуска")),
			can_exclude: true,
			uses_tag_style: true,
			options: borrowed(&[
				"Анонс",
				"Онгоинг",
				"Выпущено",
				"Приостановлено",
				"Отменено",
			]),
			ids: Some(borrowed(&[
				"ANNOUNCE",
				"ONGOING",
				"FINISHED",
				"HIATUS",
				"CANCELLED",
			])),
			..Default::default()
		}
		.into(),
	);
	out.push(
		MultiSelectFilter {
			id: Cow::Borrowed("translationStatus"),
			title: Some(Cow::Borrowed("Статус перевода")),
			can_exclude: true,
			uses_tag_style: true,
			options: borrowed(&[
				"Переводится",
				"Завершён",
				"Заморожен",
				"Заброшен",
			]),
			ids: Some(borrowed(&[
				"IN_PROGRESS",
				"FINISHED",
				"FROZEN",
				"ABANDONED",
			])),
			..Default::default()
		}
		.into(),
	);
	out.push(
		MultiSelectFilter {
			id: Cow::Borrowed("rating"),
			title: Some(Cow::Borrowed("Возрастное ограничение")),
			can_exclude: true,
			uses_tag_style: true,
			options: borrowed(&["0+", "12+", "16+", "18+"]),
			ids: Some(borrowed(&["GENERAL", "SENSITIVE", "QUESTIONABLE", "EXPLICIT"])),
			..Default::default()
		}
		.into(),
	);
	out
}

/// Map of Senkuro's `rootId` values to the user-facing group title shown in Aidoku
/// filter UI. Order here is the order they appear under the static filters.
pub const VISIBLE_ROOTS: &[(&str, &str)] = &[
	("TEFCRUw6Nw", "Демография"),
	("TEFCRUw6NQ", "Темы"),
	("TEFCRUw6NA", "Сеттинг"),
	("TEFCRUw6Mw", "Черты"),
	("TEFCRUw6Ng", "Элементы"),
];

/// Build genre filters from the API's full label list. Each Senkuro `rootId`
/// becomes its own multi-select group; slugs in `exclude_genres` are filtered out
/// (Senkuro hides hentai/yaoi/etc.; Senkognito leaves them visible).
pub fn dynamic_genre_filters(
	labels: &[LabelDto],
	exclude_genres: &[&'static str],
) -> Vec<Filter> {
	let mut out: Vec<Filter> = Vec::new();
	for (root_id, group_title) in VISIBLE_ROOTS {
		let mut options: Vec<Cow<'static, str>> = Vec::new();
		let mut ids: Vec<Cow<'static, str>> = Vec::new();
		for l in labels {
			if l.root_id.as_deref() != Some(*root_id) {
				continue;
			}
			if exclude_genres.iter().any(|g| *g == l.slug.as_str()) {
				continue;
			}
			options.push(Cow::Owned(l.display_name()));
			ids.push(Cow::Owned(l.slug.clone()));
		}
		if options.is_empty() {
			continue;
		}
		out.push(
			MultiSelectFilter {
				id: Cow::Owned(format!("label_{root_id}")),
				title: Some(Cow::Borrowed(*group_title)),
				is_genre: true,
				can_exclude: true,
				uses_tag_style: true,
				options,
				ids: Some(ids),
				..Default::default()
			}
			.into(),
		);
	}
	out
}

fn borrowed(items: &[&'static str]) -> Vec<Cow<'static, str>> {
	items.iter().map(|s| Cow::Borrowed(*s)).collect()
}
