# SynoPlayer - CLI Audio Player for Synology Audio Station

## Project Overview
CLI аудиоплеер, работающий с Synology Audio Station через Web API.
Конечная цель: запуск на одноплатнике в составе умной колонки с голосовым управлением.

## Tech Stack
- **Language**: Rust (edition 2024)
- **Async runtime**: tokio
- **HTTP client**: reqwest (with cookie support)
- **CLI framework**: clap (derive)
- **Audio playback**: rodio + symphonia (decoding)
- **TUI** (future): ratatui
- **Serialization**: serde + serde_json
- **Config**: toml (via serde) — файл `~/.config/synoplayer/config.toml`
- **Logging**: tracing + tracing-subscriber
- **Error handling**: thiserror + anyhow
- **Credentials**: keyring (OS keychain) + fallback to encrypted file
- **Audio cache**: disk-based LRU cache

## Architecture
```
src/
├── main.rs              # Entry point, CLI parsing
├── cli/                 # CLI commands (clap subcommands)
│   ├── mod.rs
│   ├── play.rs
│   ├── playlist.rs
│   ├── library.rs
│   ├── search.rs
│   └── config.rs
├── api/                 # Synology API client
│   ├── mod.rs
│   ├── client.rs        # HTTP client wrapper, auth
│   ├── auth.rs          # Login/logout, session management
│   ├── song.rs          # SYNO.AudioStation.Song
│   ├── album.rs         # SYNO.AudioStation.Album
│   ├── artist.rs        # SYNO.AudioStation.Artist
│   ├── playlist.rs      # SYNO.AudioStation.Playlist
│   ├── stream.rs        # SYNO.AudioStation.Stream
│   ├── search.rs        # SYNO.AudioStation.Search
│   ├── pin.rs           # SYNO.AudioStation.Pin (favorites)
│   ├── cover.rs         # SYNO.AudioStation.Cover
│   ├── lyrics.rs        # SYNO.AudioStation.Lyrics
│   ├── radio.rs         # SYNO.AudioStation.Radio
│   └── types.rs         # Shared API types
├── player/              # Audio playback engine
│   ├── mod.rs
│   ├── engine.rs        # rodio-based playback
│   ├── queue.rs         # Play queue management
│   └── state.rs         # Player state (playing, paused, etc.)
├── cache/               # Audio cache
│   ├── mod.rs
│   ├── manager.rs       # Cache lifecycle, eviction (LRU)
│   └── storage.rs       # Disk I/O, integrity checks
├── config/              # Configuration
│   ├── mod.rs
│   └── model.rs
├── credentials/         # Secure credential storage
│   ├── mod.rs
│   └── store.rs         # Keyring / encrypted file backend
└── error.rs             # Error types
```

## Development Methodology: TDD
- **Tests first**: тесты пишутся ДО реализации
- Новые тесты создаются с `#[ignore]`, ignore снимается при реализации модуля
- Цикл: Red (тесты падают) → Green (минимальная реализация) → Refactor
- Fixtures для API ответов в `tests/fixtures/*.json`
- Mock HTTP через `wiremock`, filesystem через `tempfile`
- Integration тесты с реальным NAS — только через env vars (`SYNO_HOST`, `SYNO_USER`, `SYNO_PASS`)
- Целевое покрытие: 95%+ для core-модулей

## Design Principles: SOLID / DRY / KISS / YAGNI / SoC

Эти принципы являются **обязательными** при написании любого кода в проекте.

### SOLID

- **S — Single Responsibility**: Каждый struct/модуль делает одно дело.
  `SynoClient` — только HTTP-транспорт. `SongApi` — только операции с песнями.
  `CacheManager` — только управление кешем, не знает про API и плеер.
- **O — Open/Closed**: Расширяем через trait-ы, не через правку существующего кода.
  Новый API endpoint = новый файл + impl trait, без изменения `client.rs`.
- **L — Liskov Substitution**: Trait implementations взаимозаменяемы.
  `CredentialStore` trait → `KeyringStore` и `EncryptedFileStore` одинаково работают для вызывающего кода.
- **I — Interface Segregation**: Trait-ы маленькие и целевые.
  Не `trait Player { fn play(); fn cache(); fn search(); }`, а отдельные trait-ы: `Playable`, `Cacheable`, `Searchable`.
- **D — Dependency Inversion**: Модули зависят от абстракций (trait), не от конкретных реализаций.
  `PlayerEngine` принимает `impl AudioSource`, а не `SynoClient` напрямую.

### DRY (Don't Repeat Yourself)

- Общая логика API-запросов — в `SynoClient::request()`, endpoint-ы только задают параметры
- Общие типы ответов — в `api/types.rs`, не дублировать в каждом endpoint-файле
- Повторяющиеся паттерны pagination/sorting — один generic helper

### KISS (Keep It Simple, Stupid)

- Простейшее решение, которое работает
- Если enum из 3 вариантов решает задачу — не строить иерархию trait-ов
- Если `String` достаточно — не делать newtype wrapper без причины
- Flat лучше nested: избегать глубокой вложенности модулей

### YAGNI (You Aren't Gonna Need It)

- Не реализовывать то, что не нужно прямо сейчас
- Не добавлять "на будущее" generic параметры, feature flags, абстракции
- Если endpoint не используется ни одной CLI-командой — не реализовывать его
- Рефакторить в абстракцию только когда дублирование реально произошло (≥3 раза)

### Separation of Concerns (SoC)

- **CLI layer**: только парсинг аргументов и форматирование вывода, никакой бизнес-логики
- **API layer**: только HTTP-взаимодействие с Synology, не знает про плеер и кеш
- **Player layer**: только воспроизведение аудио, получает bytes — не знает откуда они
- **Cache layer**: только хранение/извлечение файлов, не знает про API и плеер
- **Config layer**: только чтение/запись конфигурации
- **Credentials layer**: только безопасное хранение учётных данных
- Связывает всё вместе только `main.rs` (composition root)

## Conventions
- Код и комментарии в коде на английском
- Документация проекта на русском
- Commit messages на английском
- Все API-вызовы async
- Каждый API endpoint — отдельный файл в `src/api/`
- Тесты рядом с кодом (`#[cfg(test)] mod tests`)
- Никаких `unwrap()` в production коде — только в тестах
- Все ошибки типизированы через `thiserror`
- При добавлении нового модуля — сначала тесты, потом реализация

## Build & Run
```bash
cargo build --release
cargo run -- --help
cargo test
```

## Config File Location
`~/.config/synoplayer/config.toml`

```toml
[server]
host = "192.168.1.100"
port = 5001
https = true
quickconnect_id = ""  # optional

[auth]
username = "user"
# password stored securely: OS keyring (default) or encrypted file
# after first `synoplayer login` — auto-reconnects without prompting
credential_store = "keyring"  # "keyring" | "encrypted_file"

[cache]
enabled = true
path = "~/.cache/synoplayer/audio"    # where cached files are stored
max_size_mb = 2048                     # max cache size, oldest evicted (LRU)
ttl_days = 30                          # how long cached files are kept
cache_on_play = true                   # cache tracks during playback
preload_playlist = false               # preload entire playlist to cache
transcode_before_cache = false         # store transcoded or original format
verify_integrity = true                # check file hash on cache hit
```
