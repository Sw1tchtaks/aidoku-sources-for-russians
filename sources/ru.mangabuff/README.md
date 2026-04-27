# MangaBuff (ru.mangabuff)

Источник для [mangabuff.ru](https://mangabuff.ru) — русская манга-платформа.

## Реализация

Чистый HTML-скрейпинг.

- **Каталог**: `GET /manga?page=N` — 30 тайлов на страницу, селектор `a.cards__item`. Заголовок в `div.cards__name`, обложка в `div.cards__img[style*="background-image"]` (вытаскивается из CSS), жанры в `div.cards__info` (через запятую).
- **Поиск**: `GET /search?query=…&page=N`
- **Карточка**: `GET /manga/{slug}` — `og:image` для обложки, `og:description` для описания.
- **Главы**: парсятся со страницы карточки, селектор `a.chapters__item`. URL = `data-attr` href = `/manga/{slug}/{vol}/{chap}`, `data-chapter`/`data-chapter-date` атрибуты.
- **Страницы (чтение)**: `GET /manga/{slug}/{vol}/{chap}` — изображения лежат в `div.reader__item img[data-src]` (lazy-loaded URL вида `https://c3.mangabuff.ru/chapters/{slug}/{vol}/{chap}/N-XXX.jpeg?TS`). Источник тащит data-src как обычный URL — без CSRF.

## Известные ограничения

- Без фильтров по жанрам/типу/возрастному рейтингу — `filters.json` пуст.
- Нет авторизации — главы с пометкой 18+ или подписка-only могут не открыться (приложение покажет пустую страницу или баннер).
- Status манги не парсится (нет стабильного индикатора в HTML), всегда `Unknown`.
- `Viewer::Webtoon` по умолчанию (вертикальная прокрутка) — большинство тайтлов корейские/китайские манхвы.
