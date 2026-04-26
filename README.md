# Aidoku Sources for Russians

Источники для [Aidoku](https://aidoku.app) (iOS/iPadOS, версия 0.7+) для русскоязычных сайтов с мангой, отсутствующих в [официальном community-репозитории](https://github.com/Aidoku-Community/sources).

## Использование

В приложении Aidoku → Settings → Source Lists → Add Source List → вставьте URL:

```
https://sw1tchtaks.github.io/aidoku-sources-for-russians/index.min.json
```

После этого источник появится в списке доступных и его можно установить.

## Источники

| Источник | Сайт | Статус |
|----------|------|--------|
| Senkuro | https://senkuro.com | бета |
| Remanga | https://remanga.org | планируется |

## Известные ограничения (Senkuro v0.1)

- Нет каталога/фильтров/сортировки. На главной показывается результат поиска по букве «а» — пока пустой запрос.
- Нет тегов/жанров в фильтрах поиска (только текстовый поиск).
- Главы берутся из основной (RU primary) ветки. Альтернативные команды переводчиков пока не выбираются.
- Если CDN или GraphQL persisted-query hash изменится у Senkuro, источник может перестать работать до обновления.

## Разработка

Каждый источник — отдельный Rust-крейт под `wasm32-unknown-unknown` через [aidoku-rs](https://github.com/Aidoku/aidoku-rs). Сборка происходит автоматически в CI (`.github/workflows/build.yaml`) при пуше в `main`:

1. `aidoku package` собирает `.aix` для каждого источника
2. `aidoku build` агрегирует их в `index.min.json`
3. Результат деплоится в ветку `gh-pages` и публикуется через GitHub Pages

Локально (если хочется собирать руками):

```bash
rustup target add wasm32-unknown-unknown
cargo install --git https://github.com/Aidoku/aidoku-rs aidoku-cli
cd sources/ru.senkuro && aidoku package
```

## Лицензия

MIT. См. [LICENSE](./LICENSE).

Этот репозиторий не аффилирован ни с владельцами сайтов-источников, ни с приложением Aidoku.
