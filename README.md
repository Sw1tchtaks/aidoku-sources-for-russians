# Aidoku Sources for Russians

Источники для [Aidoku](https://aidoku.app) (iOS/iPadOS, версия 0.7+) для русскоязычных сайтов с мангой.

## Использование

В приложении Aidoku → Settings → Source Lists → Add Source List → вставьте URL:

```
https://sw1tchtaks.github.io/aidoku-sources-for-russians/index.min.json
```

После этого источники появятся в списке доступных и их можно установить.

## Источники

| Источник | Сайт | Статус | Содержимое |
|----------|------|--------|------------|
| Senkuro | https://senkuro.com | работает | манга, манхва, комиксы |
| Senkognito | https://senkognito.com | работает | 18+ |
| ReadManga | https://readmanga.live | планируется | манга |
| MintManga | https://mintmanga.live | планируется | манга |
| SelfManga | https://selfmanga.live | планируется | манга |
| AllHentai | https://allhen.online | планируется | 18+ |
| Acomics | https://acomics.ru | планируется | комиксы |
| MangaBuff | https://mangabuff.ru | планируется | манга |
| Remanga | https://remanga.org | планируется | манга |

## Что есть в официальном репозитории

Эти источники не дублируем — берите из [aidoku-community/sources](https://github.com/Aidoku-Community/sources):

- **MangaLib** (`ru.mangalib`)
- **HentaiLib** (`ru.hentailib`)
- **Desu** (`ru.desu`)

## Разработка

Каждый источник — Rust-крейт под `wasm32-unknown-unknown` через [aidoku-rs](https://github.com/Aidoku/aidoku-rs). Логика, общая для нескольких сайтов одного движка, вынесена в `templates/<engine>`:

- `templates/senkuro` — GraphQL-движок Senkuro/Senkognito.

Сборка автоматическая в CI (`.github/workflows/build.yaml`) при пуше в `main`:

1. `aidoku package` собирает `.aix` для каждого источника
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
│   └── senkuro/           # GraphQL Senkuro / Senkognito
└── sources/               # сами источники, каждый собирается в .aix
    ├── ru.senkuro/
    └── ru.senkognito/
```

## Лицензия

MIT. См. [LICENSE](./LICENSE).

Этот репозиторий не аффилирован ни с владельцами сайтов-источников, ни с приложением Aidoku.
