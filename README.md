# Aidoku Sources for Russians

[![Добавить в Aidoku](https://img.shields.io/badge/%D0%94%D0%BE%D0%B1%D0%B0%D0%B2%D0%B8%D1%82%D1%8C%20%D0%B2-Aidoku-ff2d55?style=for-the-badge)](https://sw1tchtaks.github.io/aidoku-sources-for-russians/)

Русскоязычные источники для [Aidoku](https://aidoku.app) на iPhone и iPad.

## Установка

Нажмите кнопку выше или добавьте список вручную: `Настройки` → `Список источников` → `+`.

```text
https://sw1tchtaks.github.io/aidoku-sources-for-russians/index.min.json
```

После установки списка выберите и установите нужные источники в Aidoku. Новые версии публикуются по тому же адресу автоматически.

## Источники

| Источник | Сайт | Версия | Статус | Содержимое |
| --- | --- | :---: | --- | --- |
| [AllHentai](sources/ru.allhentai/) | [20.allhen.online](https://20.allhen.online) | v7 | beta, вход через WebView | манга 18+ |
| [Senkuro](sources/ru.senkuro/) | [senkuro.com](https://senkuro.com) | v16 | работает | манга, манхва, маньхуа, комиксы |
| [Senkognito](sources/ru.senkognito/) | [senkognito.com](https://senkognito.com) | v15 | работает | отдельный хентай-каталог Senkognito |
| [ReadManga](sources/ru.readmanga/) | [a.zazaza.me](https://a.zazaza.me) | v8 | beta, вход через WebView | манга |
| [InkStory](sources/ru.inkstory/) | [inkstory.net](https://inkstory.net) | v2 | beta, вход через WebView | манга, манхва, маньхуа |
| [MangaBuff](sources/ru.mangabuff/) | [mangabuff.ru](https://mangabuff.ru) | v6 | beta | манга, манхва, маньхуа |
| [Ranobes](sources/ru.ranobes/) | [ranobes.com](https://ranobes.com) | v7 | ограничен Cloudflare | ранобэ |
| [RanobeHub](sources/ru.ranobehub/) | [ranobehub.org](https://ranobehub.org) | v2 | beta | ранобэ |
| [Ранобэ.рф](sources/ru.ranoberf/) | [ранобэ.рф](https://ранобэ.рф) | v4 | beta | ранобэ |

## Россия и VPN

Проверено 3 августа 2026 года через два узла в России, а также узлы в Германии и США. Проверялись рабочие страницы каталога или API, а не только DNS.

| Источник | Россия | Вне России / VPN | Что выбрать |
| --- | --- | --- | --- |
| AllHentai | `20.allhen.online` | тот же домен | менять адрес вручную только при официальном переезде |
| Senkuro | `api.senkuro.me` | `api.senkuro.com` | источник автоматически пробует оба API |
| Senkognito | `api.senkognito.com`, резерв `api.senkuro.com` | те же адреса | отдельного рабочего `.me` API нет; оба адреса используют `App-Id` Senkognito |
| ReadManga | `a.zazaza.me` | `a.zazaza.me` или `readmanga.me` | старые `readmanga.live` и `readmanga.ru` удалены как нерабочие |
| InkStory | `inkstory.net` + `api.inkstory.net` | те же домены | региональное зеркало не требуется |
| MangaBuff | `mangabuff.ru`, резервные `wss` | `mangabuff.ru` | `wss` извне часто возвращают `403` и работают медленнее |
| Ranobes | `ranobes.com` | тот же домен | сайт доступен, но Cloudflare может показать проверку вместо каталога |
| RanobeHub | `ranobehub.org` | тот же домен | региональное зеркало не требуется |
| Ранобэ.рф | `ранобэ.рф` | тот же домен | региональное зеркало не требуется |

Результат может отличаться у конкретного провайдера или VPN. Выбор домена и авторизация находятся в настройках соответствующего источника.

## Разработка

Исходники находятся в `sources/<id>`, общий код — в `templates/`. Пакеты собираются через [aidoku-rs](https://github.com/Aidoku/aidoku-rs).

```bash
rustup target add wasm32-unknown-unknown
cargo install --git https://github.com/Aidoku/aidoku-rs aidoku-cli
cd sources/ru.senkuro
aidoku package
```

Пуш в `main` запускает GitHub Actions, обновляет `.aix`, индекс источников и GitHub Pages.

## Лицензия

MIT. Репозиторий не связан с владельцами сайтов или разработчиками Aidoku.
