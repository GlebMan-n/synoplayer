# AGENTS.md — Руководство для AI-агентов по разработке SynoPlayer

## Обзор проекта

**SynoPlayer** — CLI аудиоплеер для Synology Audio Station. Подключается к NAS по сети,
стримит музыку, управляет плейлистами, избранным и рейтингами. Написан на Rust.

### Ключевые цели
1. **MVP**: Воспроизведение музыки из Audio Station через CLI
2. **Полная версия**: Все функции веб-плеера Audio Station
3. **Финальная цель**: Работа на одноплатнике как умная колонка с голосовым управлением

---

## Архитектура

### Слои приложения

```
┌─────────────────────────────────────────────┐
│                CLI Layer (clap)              │
│  Парсинг команд, форматирование вывода      │
├─────────────────────────────────────────────┤
│              Player Engine (rodio)           │
│  Воспроизведение, очередь, состояние         │
├──────────────────┬──────────────────────────┤
│  Audio Cache     │  Synology API Client     │
│  LRU disk cache  │  HTTP, auth, endpoints   │
├──────────────────┴──────────────────────────┤
│         Config, Credentials & Storage        │
│  TOML конфиг, keyring/encrypted creds, кеш  │
└─────────────────────────────────────────────┘
```

### Принципы проектирования: SOLID / DRY / KISS / YAGNI / SoC

Подробно описаны в `CLAUDE.md`. Ниже — как они применяются к архитектуре:

**Separation of Concerns** — каждый слой изолирован:
- CLI layer не содержит бизнес-логику, только парсинг и вывод
- API layer не знает о плеере и кеше
- Player layer получает `impl AudioSource` — не знает откуда bytes
- Cache layer не знает про API и плеер
- Связывает всё `main.rs` (composition root)

**Dependency Inversion** — зависимости через trait-ы:
```rust
// Player не зависит от SynoClient напрямую
trait AudioSource: Send + Sync {
    async fn stream(&self, song_id: &str) -> Result<impl AsyncRead>;
}

// CacheManager оборачивает любой AudioSource
// и сам является AudioSource (decorator pattern)
impl AudioSource for CachedSource { ... }
```

**Single Responsibility** — один struct = одна ответственность:
```
SynoClient     → HTTP транспорт + сессия
SongApi        → CRUD операции с песнями
CacheManager   → LRU/TTL логика кеша
CacheStorage   → дисковый I/O
PlayerEngine   → rodio Sink управление
PlayQueue      → порядок воспроизведения
```

**YAGNI** — реализуем только то, что нужно текущему этапу:
- Этап 1 не реализует Genre/Composer API — они нужны только в Этапе 3
- Не создавать абстракции "на вырост" — рефакторить когда дублирование реально случится
- Если одна реализация trait достаточна — не делать trait, просто struct

**KISS** — простейшее решение:
- `enum PlayerState { Stopped, Playing, Paused }` — не иерархия классов
- Config — один плоский TOML, не система profiles/environments
- Ошибки — `thiserror` enum, не дерево типов

**DRY** — общий код выделяется, но не заранее:
- `SynoClient::request()` — один метод для всех API-вызовов
- `ApiResponse<T>` — один generic тип ответа
- Pagination — один helper, не копипаста в каждом endpoint

Другие принципы:
- **Async everywhere**: Все I/O через tokio
- **Fail gracefully**: Потеря сети → пауза и retry, не crash
- **Minimal dependencies**: Не тянуть лишнее

---

## Synology Audio Station Web API

### Аутентификация

1. Вызов `SYNO.API.Info` (`query=all`) для обнаружения путей
2. Вызов `SYNO.API.Auth` method=`login` → получить `sid`
3. Все последующие запросы с параметром `_sid=<sid>`
4. При завершении — `logout`

**Base URL**: `https://<host>:<port>/webapi/<path>`

### Основные API endpoints

| API | Методы | Описание |
|-----|--------|----------|
| `SYNO.API.Info` | `query` | Обнаружение всех API |
| `SYNO.API.Auth` | `login`, `logout` | Аутентификация |
| `SYNO.AudioStation.Info` | `getinfo` | Информация о Audio Station |
| `SYNO.AudioStation.Song` | `list`, `search`, `getinfo`, `setrating` | Управление песнями |
| `SYNO.AudioStation.Album` | `list` | Список альбомов |
| `SYNO.AudioStation.Artist` | `list` | Список артистов |
| `SYNO.AudioStation.Playlist` | `list`, `getinfo`, `create`, `delete`, `rename`, `updatesongs`, `createsmart` | Плейлисты |
| `SYNO.AudioStation.Stream` | `stream`, `transcode` | Стриминг аудио |
| `SYNO.AudioStation.Search` | `list` | Глобальный поиск |
| `SYNO.AudioStation.Pin` | `pin`, `unpin`, `list`, `rename`, `reorder` | Избранное |
| `SYNO.AudioStation.Cover` | `getsongcover`, `getfoldercover` | Обложки |
| `SYNO.AudioStation.Lyrics` | `getlyrics`, `setlyrics` | Тексты песен |
| `SYNO.AudioStation.Radio` | `list`, `add`, `search` | Интернет-радио |
| `SYNO.AudioStation.Folder` | `list`, `getinfo` | Навигация по папкам |
| `SYNO.AudioStation.Genre` | `list` | Жанры |
| `SYNO.AudioStation.Composer` | `list` | Композиторы |
| `SYNO.AudioStation.Download` | `download` | Скачивание файлов |

### Формат запросов

```
GET /webapi/AudioStation/song.cgi?api=SYNO.AudioStation.Song&version=3&method=list&offset=0&limit=50&additional=song_tag,song_audio,song_rating&_sid=<sid>
```

### Формат ответов

```json
{
  "success": true,
  "data": {
    "songs": [...],
    "total": 1234,
    "offset": 0
  }
}
```

### Стриминг

```
GET /webapi/AudioStation/stream.cgi/0.mp3?api=SYNO.AudioStation.Stream&version=2&method=stream&id=<song_id>&_sid=<sid>
```

Ответ — бинарный аудиопоток. Для транскодирования: `method=transcode`.

### Рейтинги

`SYNO.AudioStation.Song` method=`setrating`, параметры: `id`, `rating` (0-5).
Получение: `additional=song_rating` в list/getinfo.

### Избранное (Pin)

`SYNO.AudioStation.Pin` — pin/unpin/list/rename/reorder.

---

## Методология: TDD (Test-Driven Development)

Разработка ведётся по принципу **тесты → реализация → рефакторинг**.

Этап 0 создаёт все тесты с `#[ignore]`. По мере реализации каждого модуля:
1. Снять `#[ignore]` с тестов для модуля
2. Запустить — красные (не компилируется / fail)
3. Написать минимальную реализацию — зелёные
4. Рефакторинг при зелёных тестах

Подробная спецификация тестов — в `docs/SPECIFICATION.md`, секция 8.

---

## Этапы разработки

### Этап 0: Тестовый фундамент (TDD)
**Цель**: Все тесты написаны до первой строки production-кода

- [ ] Инициализация Cargo проекта (Cargo.toml с deps + dev-deps)
- [ ] Структура модулей — пустые файлы с pub trait/struct заглушками
- [ ] Тестовые fixtures: JSON-файлы с ответами API (`tests/fixtures/`)
- [ ] Unit-тесты API parsing (все типы ответов, все error codes)
- [ ] Unit-тесты API client (wiremock: login, relogin, discovery, requests)
- [ ] Unit-тесты Player state machine (все переходы состояний)
- [ ] Unit-тесты PlayQueue (add, remove, next, prev, shuffle, repeat modes)
- [ ] Unit-тесты CacheManager (LRU, TTL, integrity, disabled, status)
- [ ] Unit-тесты CredentialStore (save, load, clear, overwrite, encrypted file)
- [ ] Unit-тесты Config (parsing, defaults, roundtrip)
- [ ] CLI integration тесты (assert_cmd: --help, ошибки, cache status)
- [ ] Integration тесты с NAS (env-gated, `#[ignore]`)
- [ ] Все тесты помечены `#[ignore]` — проект компилируется, 0 тестов запускается

### Этап 1: Фундамент (MVP-подготовка)
**Цель**: Рабочий скелет проекта (снимаем `#[ignore]` с тестов по мере реализации)

- [ ] Модель конфигурации (`config.toml`) с serde → снять ignore с config тестов
- [ ] Обработка ошибок (error types)
- [ ] Базовый HTTP-клиент с поддержкой HTTPS → снять ignore с client тестов
- [ ] Аутентификация: login/logout, хранение сессии → снять ignore с auth тестов
- [ ] **Сохранение учётных данных** (keyring / encrypted file) → снять ignore с credential тестов
- [ ] API discovery через `SYNO.API.Info`
- [ ] Базовая структура CLI (clap subcommands) → снять ignore с CLI тестов

### Этап 2: MVP — Воспроизведение
**Цель**: Слушать музыку из командной строки

- [ ] Получение списка песен (`Song.list`)
- [ ] Поиск песен (`Song.search`, `Search.list`)
- [ ] Стриминг аудио (`Stream.stream`) в rodio
- [ ] Базовые команды: `play`, `pause`, `stop`, `next`, `prev`
- [ ] Отображение текущего трека (название, артист, длительность)
- [ ] Управление громкостью
- [ ] Очередь воспроизведения (queue)

### Этап 3: Библиотека и навигация
**Цель**: Полноценный просмотр музыкальной коллекции

- [ ] Список альбомов (`Album.list`)
- [ ] Список артистов (`Artist.list`)
- [ ] Список жанров (`Genre.list`)
- [ ] Список композиторов (`Composer.list`)
- [ ] Навигация по папкам (`Folder.list`)
- [ ] Обложки альбомов (`Cover.getsongcover`) — кеширование

### Этап 4: Плейлисты
**Цель**: Полное управление плейлистами

- [ ] Список плейлистов (`Playlist.list`)
- [ ] Просмотр содержимого плейлиста (`Playlist.getinfo`)
- [ ] Создание плейлиста (`Playlist.create`)
- [ ] Удаление плейлиста (`Playlist.delete`)
- [ ] Переименование (`Playlist.rename`)
- [ ] Добавление/удаление песен (`Playlist.updatesongs`)
- [ ] Умные плейлисты (`Playlist.createsmart`)
- [ ] Воспроизведение плейлиста целиком

### Этап 5: Рейтинги и избранное
**Цель**: Персонализация

- [ ] Установка рейтинга трека (`Song.setrating`, 0-5)
- [ ] Отображение рейтинга в списках
- [ ] Добавление в избранное (`Pin.pin`)
- [ ] Удаление из избранного (`Pin.unpin`)
- [ ] Список избранного (`Pin.list`)

### Этап 6: Кеширование аудио
**Цель**: Оффлайн-воспроизведение, экономия трафика

- [ ] Модуль кеша: `cache/manager.rs`, `cache/storage.rs`
- [ ] LRU-вытеснение при превышении max_size_mb
- [ ] Автокеширование при воспроизведении (`cache_on_play`)
- [ ] Предзагрузка плейлиста (`preload_playlist`)
- [ ] TTL: автоудаление старых файлов по `ttl_days`
- [ ] Проверка целостности (SHA-256 hash)
- [ ] CLI: `synoplayer cache status`, `cache clear`, `cache preload <playlist>`
- [ ] Воспроизведение из кеша при недоступности сервера

### Этап 7: Дополнительные функции
**Цель**: Паритет с веб-плеером

- [ ] Тексты песен (`Lyrics.getlyrics`)
- [ ] Интернет-радио (`Radio.list`, `Radio.add`)
- [ ] Shuffle / Repeat режимы
- [ ] Эквалайзер (если поддерживает rodio/symphonia)
- [ ] История воспроизведения (локально)
- [ ] Скачивание треков (`Download.download`)

### Этап 8: TUI (Text User Interface)
**Цель**: Удобный интерфейс в терминале

- [ ] ratatui-based TUI
- [ ] Визуализация очереди
- [ ] Навигация по библиотеке
- [ ] Прогресс-бар воспроизведения
- [ ] Обложки в терминале (sixel/kitty protocol)

### Этап 9: Одноплатник и голос (будущее)
**Цель**: Умная колонка

- [ ] Cross-compilation для ARM (aarch64)
- [ ] Systemd service
- [ ] Интеграция с STT (Speech-to-Text)
- [ ] Голосовые команды
- [ ] Wake word detection

---

## CLI-команды (целевой интерфейс)

```bash
# Первоначальная настройка (один раз)
synoplayer config set-server 192.168.1.100
synoplayer config set-port 5001
synoplayer login                  # вводишь логин/пароль, сохраняется в keyring
                                  # дальше auto-connect при любой команде

# Управление учётными данными
synoplayer login --save           # сохранить в keyring (по умолчанию)
synoplayer login --no-save        # одноразовая сессия
synoplayer credentials clear      # удалить сохранённые credentials

# Воспроизведение
synoplayer play <song_id_or_name>
synoplayer pause
synoplayer resume
synoplayer stop
synoplayer next
synoplayer prev
synoplayer volume <0-100>
synoplayer seek <time>
synoplayer now                    # что сейчас играет
synoplayer queue                  # очередь

# Библиотека
synoplayer songs [--album X] [--artist X] [--genre X] [--limit N]
synoplayer albums [--artist X]
synoplayer artists
synoplayer genres
synoplayer folders [path]
synoplayer search <keyword>

# Плейлисты
synoplayer playlists
synoplayer playlist <name>        # содержимое
synoplayer playlist create <name>
synoplayer playlist delete <name>
synoplayer playlist add <playlist> <song_id>
synoplayer playlist remove <playlist> <song_id>
synoplayer playlist play <name>

# Рейтинг и избранное
synoplayer rate <song_id> <1-5>
synoplayer favorite <song_id>
synoplayer unfavorite <song_id>
synoplayer favorites

# Тексты
synoplayer lyrics [song_id]

# Радио
synoplayer radio list
synoplayer radio play <station>
synoplayer radio add <name> <url>

# Режимы
synoplayer shuffle [on|off]
synoplayer repeat [off|one|all]

# Кеш
synoplayer cache status           # размер, кол-во файлов, лимит
synoplayer cache clear            # очистить весь кеш
synoplayer cache clear --older 30d  # удалить старше 30 дней
synoplayer cache preload <playlist> # предзагрузить плейлист
synoplayer cache list             # список закешированных треков

# Logout
synoplayer logout
```

---

## Важные технические детали

### Стриминг и буферизация
- Аудио стримится через HTTP GET, ответ — бинарный поток
- Нужна буферизация: скачивать чанками и подавать в rodio
- При потере сети — пауза, retry с того же места (если сервер поддерживает Range)
- symphonia обеспечивает декодирование всех форматов (FLAC, MP3, AAC, WAV, OGG)

### Фоновое воспроизведение
- Плеер работает как фоновый процесс (daemon mode) или в foreground
- CLI команды общаются с daemon через Unix socket или файл состояния
- Для MVP: foreground-only, один процесс

### Учётные данные и автоподключение
- Первый `synoplayer login` — интерактивный ввод логина/пароля
- Credentials сохраняются в OS keyring (Linux: Secret Service/libsecret, macOS: Keychain)
- Fallback: зашифрованный файл `~/.config/synoplayer/credentials.enc` (AES-256-GCM, ключ из machine-id)
- Настройка: `credential_store = "keyring" | "encrypted_file"` в config.toml
- Все последующие команды — автоматический login без ввода пароля
- `sid` кешировать на диск (`~/.config/synoplayer/session.json`)
- При ошибке 106/119 — auto-relogin из сохранённых credentials
- `synoplayer credentials clear` — удалить сохранённые данные
- Зависимость: crate `keyring` (кроссплатформенный, поддерживает Linux/macOS/Windows)

### Кеширование аудио
- Каталог кеша: `~/.cache/synoplayer/audio/` (настраивается в `[cache]` секции config)
- Структура: `<cache_path>/<song_id_hash>.audio` + `<song_id_hash>.meta` (JSON с метаданными)
- **LRU-вытеснение**: при превышении `max_size_mb` удаляются наименее недавно использованные файлы
- **TTL**: файлы старше `ttl_days` удаляются при запуске / периодической очистке
- **Целостность**: SHA-256 хеш сохраняется в .meta, проверяется при чтении (`verify_integrity`)
- **cache_on_play**: при воспроизведении трек параллельно сохраняется на диск
- **preload_playlist**: фоновая загрузка всех треков плейлиста
- **Оффлайн-режим**: если сервер недоступен, воспроизведение из кеша
- **transcode_before_cache**: кешировать оригинал (default) или транскодированный MP3

Настройки в config.toml:
```toml
[cache]
enabled = true
path = "~/.cache/synoplayer/audio"
max_size_mb = 2048            # 2 GB по умолчанию
ttl_days = 30                 # 30 дней по умолчанию
cache_on_play = true          # кешировать при воспроизведении
preload_playlist = false      # предзагрузка плейлистов
transcode_before_cache = false # кешировать оригинал или MP3
verify_integrity = true       # проверять хеш
```

### Подключение
- Поддержка HTTPS (по умолчанию порт 5001) и HTTP (порт 5000)
- Опция отключения проверки сертификата (self-signed)
- QuickConnect: для будущих версий

---

## Для агентов: правила работы

1. **Читай CLAUDE.md** перед началом работы — там конвенции и структура
2. **Не меняй архитектуру** без обсуждения с пользователем
3. **Один PR = один этап** (или подэтап)
4. **TDD обязателен**: при добавлении нового модуля — сначала тесты (с `#[ignore]`), потом реализация
5. **Снимай `#[ignore]`** только когда реализуешь соответствующий модуль
6. **Не пиши production-код без теста** — каждая публичная функция покрыта
7. **Документируй публичные API** модулей через rustdoc
8. **Cargo clippy** должен проходить без warnings
9. **Cargo fmt** перед каждым коммитом
10. **`cargo test`** должен проходить перед каждым коммитом (ignored тесты не считаются)
11. При работе с API — всегда проверяй `"success": true` в ответе
12. Все строки от сервера — UTF-8, учитывай не-ASCII символы в названиях
13. Помни: официальной документации API нет, поведение может отличаться — логируй ответы

### Anti-patterns (запрещено)

- **God object**: struct с 10+ полями и 20+ методами → разбить на отдельные struct-ы
- **Leaky abstraction**: CLI-хендлер вызывает `reqwest::get()` напрямую → только через `SynoClient`
- **Premature abstraction**: trait с одной реализацией "на будущее" → просто struct
- **Shotgun surgery**: изменение одной фичи требует правок в 5+ файлах → пересмотреть границы модулей
- **Feature envy**: метод `PlayerEngine` читает поля `CacheManager` → переместить логику
- **Copy-paste endpoints**: 10 файлов API с одинаковым кодом пагинации → выделить в helper
- **Stringly typed**: `fn set_repeat(mode: &str)` → `fn set_repeat(mode: RepeatMode)`
- **Boolean blindness**: `fn play(song, true, false, true)` → использовать enum / builder
