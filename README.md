# Aidoku Sources for Russians

Источники для [Aidoku](https://aidoku.app) (iOS/iPadOS, версия 0.7+) для русскоязычных сайтов с мангой и ранобэ.

## Использование

В приложении Aidoku → Settings → Source Lists → Add Source List → вставьте URL:

```
https://sw1tchtaks.github.io/aidoku-sources-for-russians/index.min.json
```

После этого источники появятся в списке доступных и их можно установить.

> Aidoku кеширует индекс на стороне приложения и иногда системно через iOS URLSession. Если после моего пуша новые источники не появляются — удалите URL из Source Lists, форс-закройте Aidoku (свайп вверх в переключателе задач) и добавьте URL заново.

## Источники

| Источник | Сайт | Версия | Статус | Содержимое |
|----------|------|:---:|--------|------------|
| [Senkuro](sources/ru.senkuro/) | https://senkuro.com | v6 | работает | манга, манхва, комиксы |
| [ReadManga](sources/ru.readmanga/) | https://readmanga.live | v1 | beta | манга (Grouple) |
| [MintManga](sources/ru.mintmanga/) | https://mintmanga.live | v1 | beta | манга (Grouple) |
| [SelfManga](sources/ru.selfmanga/) | https://selfmanga.live | v1 | beta | манга (Grouple) |
| [MangaBuff](sources/ru.mangabuff/) | https://mangabuff.ru | v1 | beta | манга, манхва |
| [Acomics](sources/ru.acomics/) | https://acomics.ru | v1 | beta | веб-комиксы |
| [Ranobes](sources/ru.ranobes/) | https://ranobes.com | v4 | beta | ранобэ (текст + иллюстрации) |
| [RanobeHub](sources/ru.ranobehub/) | https://ranobehub.org | v2 | beta | ранобэ (текст) |
| [Ранобэ.рф](sources/ru.ranoberf/) | https://ранобэ.рф | v2 | beta | ранобэ (текст) |

**Beta** означает, что источник собирается и устанавливается, но полевая обкатка ещё не закончена. Если что-то не работает — заведите [issue](https://github.com/Sw1tchtaks/aidoku-sources-for-russians/issues).

### Возможности по семействам

- **Senkuro / Senkognito** — общий GraphQL-движок (`templates/senkuro`):
  - Главная-вид с большой каруселью «Популярное» + ленты по типу контента (Манга / Манхва / Маньхуа / Комиксы)
  - Динамические фильтры жанров (~1100 меток, подгружаются с API при первом открытии каталога) сгруппированы по разделам Демография / Темы / Сеттинг / Черты / Элементы
  - Статические фильтры тип / формат / статус / статус перевода / возрастной рейтинг
  - Пагинация каталога 10/страницу (хардкод сервера)
  - Senkognito по умолчанию подмешивает `rating include EXPLICIT`, поэтому каталог реально показывает 18+ контент, а не дубль Senkuro
  - Веб-логин не реализован: API анонимный, для бесплатных тайтлов работает без аккаунта
- **Grouple-семейство** (ReadManga / MintManga / SelfManga / AllHentai) — общий HTML-парсер (`templates/grouple`):
  - Каталог + поиск
  - Современный (`.cr-*`) и легаси (`.expandable`) layout карточек
  - Извлечение страниц чтения из `rm_h.readerInit(...)` JS-массива
  - Зеркало настраивается в Settings источника
  - Платные/лицензированные главы и WebView-логин в v1 не поддерживаются
- **Ранобэ-источники** (Ranobes / RanobeHub / Ранобэ.рф):
  - Главы возвращаются как `PageContent::Text(markdown)` и рендерятся в вертикальном тексто-ридере
  - **Ranobes** — DLE-сайт за DDoS-Guard, HTML-скрейпинг. Список глав ограничен 10 страницами (≈250 глав) чтобы не получить 403. Картиночные главы (иллюстрации) пока не отображаются — встроенные `<img>` теряются при strip-HTML.
  - **RanobeHub** — чистый JSON REST API на `/api/{search,ranobe/{id},ranobe/{id}/contents,chapter/{id}}`, страницы поиска по 12 элементов с настоящей пагинацией.
  - **Ранобэ.рф** — Next.js SPA. Каталог через `/v3/book` тащит сразу все 800 книг (~1.1 МБ), пагинация на стороне приложения. Карточка и текст глав — парсинг `__NEXT_DATA__` JSON. Без обложек в каталоге (cover URL содержит per-book upload-id, доступный только на странице книги).
  - У всех — Home-вид как у Senkuro/MangaLib: большая карусель «Популярное» + горизонтальная лента «Каталог».

## Что есть в официальном репозитории

Эти источники не дублируем — берите из [aidoku-community/sources](https://github.com/Aidoku-Community/sources):

- **MangaLib** (`ru.mangalib`)
- **HentaiLib** (`ru.hentailib`)
- **Desu** (`ru.desu`)
- **RanobeLib** (`ru.ranobelib`) — для ранобэ через LibGroup
- **SlashLib** (`ru.slashlib`)

## Разработка

Каждый источник — Rust-крейт под `wasm32-unknown-unknown` через [aidoku-rs](https://github.com/Aidoku/aidoku-rs). Логика, общая для нескольких сайтов одного движка, вынесена в `templates/<engine>`:

- `templates/senkuro` — GraphQL-движок Senkuro / Senkognito
- `templates/grouple` — HTML-парсер ReadManga-family

Сборка автоматическая в CI (`.github/workflows/build.yaml`) при пуше в `main` или `templates/`:

1. `aidoku package` собирает `.aix` для каждого источника в `sources/`
2. `aidoku build` агрегирует их в `index.min.json`
3. Результат деплоится в ветку `gh-pages` и публикуется через GitHub Pages

Локально:

```bash
rustup target add wasm32-unknown-unknown
cargo install --git https://github.com/Aidoku/aidoku-rs aidoku-cli
cd sources/ru.senkuro && aidoku package
```

## Структура репозитория

```
.
├── templates/             # переиспользуемые движки (path-зависимости)
│   ├── senkuro/           # GraphQL Senkuro / Senkognito
│   └── grouple/           # HTML ReadManga / MintManga / SelfManga / AllHentai
└── sources/               # сами источники, каждый собирается в .aix
    ├── ru.senkuro/
    ├── ru.senkognito/
    ├── ru.readmanga/
    ├── ru.mintmanga/
    ├── ru.selfmanga/
    ├── ru.allhentai/
    └── ru.ranobes/
```

## Лицензия

MIT. См. [LICENSE](./LICENSE).

Этот репозиторий не аффилирован ни с владельцами сайтов-источников, ни с приложением Aidoku.
