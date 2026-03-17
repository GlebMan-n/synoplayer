# SynoPlayer

CLI аудиоплеер для Synology Audio Station.

## Что это

Утилита командной строки для воспроизведения музыки с Synology NAS через Audio Station Web API.
Подключается к NAS по локальной сети (или QuickConnect), стримит аудио и управляет воспроизведением.

## Возможности (план)

- Воспроизведение музыки с Synology Audio Station
- Управление: play, pause, stop, next, prev, volume, seek
- Навигация по библиотеке: песни, альбомы, артисты, жанры, папки
- Поиск по всей коллекции
- Работа с плейлистами: просмотр, создание, редактирование, умные плейлисты
- Рейтинги треков (1-5 звёзд)
- Избранное (pin/unpin)
- Тексты песен
- Интернет-радио
- Shuffle / Repeat режимы
- Эквалайзер

## Конечная цель

Запуск на одноплатнике (Raspberry Pi / Orange Pi) в составе умной колонки с голосовым управлением.

## Технологии

- **Rust** — основной язык
- **tokio** — async runtime
- **reqwest** — HTTP client
- **clap** — CLI framework
- **rodio + symphonia** — воспроизведение аудио
- **serde** — сериализация

## Структура проекта

```
├── Cargo.toml
├── CONVENTIONS.md                   # Конвенции для AI-агентов
├── AGENTS.md                   # Руководство по разработке
├── README.md
├── docs/
│   ├── API_REFERENCE.md        # Справочник Synology API
│   ├── SPECIFICATION.md        # Спецификация разработки
│   ├── AS_Guide.pdf            # Synology Lyrics Module Guide
│   └── DSM_Developer_Guide_7_enu.pdf
├── src/
│   ├── main.rs                 # Entry point, CLI dispatch
│   ├── lib.rs                  # Публичные модули
│   ├── error.rs                # Типы ошибок
│   ├── api/                    # Synology API client
│   │   ├── client.rs           # HTTP transport, сессии
│   │   ├── auth.rs             # Login / logout / API discovery
│   │   ├── song.rs             # Песни (list, search, rating)
│   │   ├── album.rs, artist.rs # Альбомы, артисты
│   │   ├── playlist.rs         # Плейлисты (CRUD)
│   │   ├── stream.rs           # Стриминг аудио
│   │   ├── pin.rs              # Избранное
│   │   └── ...                 # cover, lyrics, radio, search
│   ├── player/
│   │   ├── engine.rs           # Воспроизведение (subprocess)
│   │   ├── state.rs            # State machine (play/pause/stop)
│   │   └── queue.rs            # Очередь, shuffle, repeat
│   ├── cache/
│   │   ├── manager.rs          # LRU-кеш, TTL, integrity
│   │   └── storage.rs          # Дисковый I/O, хеширование
│   ├── config/
│   │   └── model.rs            # TOML конфигурация
│   └── credentials/
│       └── store.rs            # Хранение учётных данных
└── tests/
    ├── api_parsing.rs          # Тесты парсинга API ответов
    ├── cli_integration.rs      # Тесты CLI
    └── fixtures/               # JSON-фикстуры Synology API
```

## Сборка из исходников

### Требования

- **Rust** 1.85+ (edition 2024) — [установка](https://rustup.rs/)
- **pkg-config** и системные библиотеки (для полной сборки с аудио):

```bash
# Debian / Ubuntu
sudo apt install pkg-config libasound2-dev libdbus-1-dev

# Fedora
sudo dnf install pkg-config alsa-lib-devel dbus-devel

# Arch
sudo pacman -S pkg-config alsa-lib dbus

# macOS — зависимости не нужны (CoreAudio / Keychain из коробки)
```

> Без `libasound2-dev` и `libdbus-1-dev` проект собирается, но без rodio (аудио через subprocess: ffplay/mpv/pw-play/paplay) и без keyring (credentials в зашифрованном файле).

### Сборка

```bash
git clone <repo-url>
cd synologyAudio

# Debug-сборка
cargo build

# Release-сборка (оптимизированная)
cargo build --release

# Бинарник будет в:
# ./target/debug/synoplayer       (debug)
# ./target/release/synoplayer     (release)
```

### Запуск тестов

```bash
# Все тесты
cargo test

# С выводом логов
RUST_LOG=debug cargo test -- --nocapture

# Только конкретный модуль
cargo test api::auth
cargo test player::queue
cargo test cache::manager

# Integration-тесты с реальным NAS (опционально)
SYNO_HOST=https://192.168.1.100:5001 SYNO_USER=admin SYNO_PASS=secret \
  cargo test integration_ -- --ignored
```

### Линтинг и форматирование

```bash
cargo fmt --check    # проверка форматирования
cargo fmt            # автоформатирование
cargo clippy         # линтер
```

### Cross-compilation (для ARM / одноплатников)

```bash
# Установить target
rustup target add aarch64-unknown-linux-gnu

# Собрать (нужен линкер, например через cross)
cargo install cross
cross build --release --target aarch64-unknown-linux-gnu
```

### Для воспроизведения аудио

На машине, где будет запускаться плеер, нужен один из:
- `ffplay` (из пакета `ffmpeg`) — рекомендуется
- `mpv`
- `pw-play` (PipeWire)
- `paplay` (PulseAudio)

```bash
# Debian / Ubuntu
sudo apt install ffmpeg   # или mpv
```

## Начало работы

```bash
# Конфигурация
synoplayer config set-server 192.168.1.100
synoplayer login

# Воспроизведение
synoplayer search "Pink Floyd"
synoplayer play <song_id>
synoplayer now
synoplayer pause
synoplayer next
```

## Этапы разработки

1. **Фундамент** — скелет проекта, HTTP клиент, аутентификация
2. **MVP** — стриминг и воспроизведение
3. **Библиотека** — навигация по коллекции
4. **Плейлисты** — полное управление
5. **Рейтинги и избранное** — персонализация
6. **Доп. функции** — lyrics, radio, equalizer
7. **TUI** — текстовый интерфейс (ratatui)
8. **Одноплатник** — cross-compilation, голосовое управление

Подробности в [AGENTS.md](AGENTS.md) и [docs/SPECIFICATION.md](docs/SPECIFICATION.md).
