# Remanga (ru.remanga)

Источник для [remanga.org](https://remanga.org).

## Реализация

JSON API на `https://api.remanga.org`. Все ответы обёрнуты в `{ "content": ... }`. Использует Tachiyomi-style endpoints:

- `GET /api/v2/search/catalog/?page=N&count=30&ordering=…` — каталог + сортировка по списку
- `GET /api/v2/search/?query=…&page=N` — текстовый поиск
- `GET /api/v2/titles/{dir}/` — карточка манги (включает `branches[]`)
- `GET /api/v2/titles/chapters/?branch_id={id}&page=N&count=200&ordering=-index` — список глав в выбранной ветке
- `GET /api/v2/titles/chapters/{id}/` — страницы главы

## Авторизация

В Settings источника:

- **Войти на Ремангу** — открывает webview с `remanga.org/login`. После успешного входа Aidoku передаёт куки в `WebLoginHandler::handle_web_login`. Источник пытается вытащить JWT из куков `access_token` / `token` / `authToken` или из URL-encoded JSON в куки `user`, и сохраняет его в defaults.
- **Auth Token (опционально)** — можно вставить токен руками, если веб-логин не подцепил.

Сохранённый токен добавляется в каждый запрос к API как `Authorization: bearer <token>`. Без него работают бесплатные главы и каталог; платные главы помечены `locked: true` в Aidoku и не открываются.

## Известные ограничения (v1)

- Фильтры по жанрам/категориям/возрасту/типу пока не реализованы (`filters.json` пуст). Каталог сортируется по рейтингу.
- Парсинг описания — простой strip HTML-тегов, не markdown.
- VPN-блок: `api.remanga.org` иногда отдаёт 403 на не-российские IP. Без VPN в РФ работает прозрачно; через VPN с зарубежным exit-нодой может потребоваться отключить туннелирование на момент сессии.
