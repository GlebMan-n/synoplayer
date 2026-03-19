# SynoPlayer

CLI аудиоплеер для Synology Audio Station.

## Что это

Утилита командной строки для воспроизведения музыки с Synology NAS через Audio Station Web API.
Подключается к NAS по локальной сети (или QuickConnect), стримит аудио и управляет воспроизведением.

Включает полнофункциональный интерактивный TUI (Text User Interface) на базе ratatui.

## Конечная цель

Запуск на одноплатнике (Raspberry Pi / Orange Pi) в составе умной колонки с голосовым управлением.

## Возможности

- Воспроизведение музыки с Synology Audio Station (стриминг по HTTP)
- Интерактивный TUI-плеер с навигацией по библиотеке
- Навигация: песни, альбомы, артисты, жанры, композиторы, папки
- Поиск по всей коллекции
- Плейлисты: просмотр, создание, удаление, переименование, импорт .m3u, умные плейлисты
- Рейтинги треков (1-5 звёзд)
- Избранное (pin/unpin)
- Тексты песен
- Интернет-радио
- Shuffle / Repeat (off, one, all) режимы
- Дисковый LRU-кеш с SHA-256 проверкой целостности
- История воспроизведения
- Загрузка треков на локальный диск

## Сборка из исходников

### Требования

- **Rust** 1.85+ (edition 2024) — [установка](https://rustup.rs/)
- **pkg-config** и системные библиотеки:

```bash
# Debian / Ubuntu
sudo apt install pkg-config libasound2-dev libdbus-1-dev

# Fedora
sudo dnf install pkg-config alsa-lib-devel dbus-devel

# Arch
sudo pacman -S pkg-config alsa-lib dbus

# macOS — зависимости не нужны (CoreAudio / Keychain из коробки)
```

### Воспроизведение аудио

На машине, где запускается плеер, нужен один из:
- `ffplay` (из пакета `ffmpeg`) — рекомендуется
- `mpv`
- `pw-play` (PipeWire)
- `paplay` (PulseAudio)

```bash
# Debian / Ubuntu
sudo apt install ffmpeg   # или: sudo apt install mpv
```

### Сборка

```bash
git clone <repo-url>
cd synologyAudio

# Debug-сборка
cargo build

# Release-сборка (оптимизированная)
cargo build --release

# Бинарник:
# ./target/debug/synoplayer       (debug)
# ./target/release/synoplayer     (release)
```

### Cross-compilation (для ARM / одноплатников)

```bash
rustup target add aarch64-unknown-linux-gnu
cargo install cross
cross build --release --target aarch64-unknown-linux-gnu
```

### Тесты

```bash
cargo test                          # все тесты
RUST_LOG=debug cargo test -- --nocapture  # с логами

# Integration-тесты с реальным NAS (опционально)
SYNO_HOST=https://192.168.1.100:5001 SYNO_USER=admin SYNO_PASS=secret \
  cargo test integration_ -- --ignored
```

---

## Быстрый старт

### 1. Настройка подключения

```bash
# Указать адрес NAS
synoplayer config set-server 192.168.1.100

# Указать порт (по умолчанию 5001)
synoplayer config set-port 5001

# Посмотреть текущую конфигурацию
synoplayer config show
```

### 2. Авторизация

```bash
# Логин (пароль запрашивается интерактивно, сохраняется в keychain)
synoplayer login

# Логин без сохранения пароля (одноразовая сессия)
synoplayer login --no-save

# Выход
synoplayer logout

# Очистка сохранённых учётных данных
synoplayer credentials clear
```

После `synoplayer login` сессия сохраняется и все последующие команды выполняются автоматически без повторного ввода пароля.

---

## Использование CLI

### Навигация по библиотеке

```bash
# Список песен (по умолчанию первые 50)
synoplayer songs
synoplayer songs --limit 200

# Фильтрация по артисту, альбому, жанру
synoplayer songs --artist "Pink Floyd"
synoplayer songs --album "The Wall"
synoplayer songs --genre "Rock" --limit 100

# Список альбомов
synoplayer albums
synoplayer albums --artist "Metallica"

# Список артистов
synoplayer artists

# Список жанров
synoplayer genres

# Список композиторов
synoplayer composers

# Навигация по папкам NAS
synoplayer folders                    # корневые папки
synoplayer folders "/music/Rock"      # содержимое подпапки
```

### Поиск

```bash
synoplayer search "Dark Side"
```

Поиск ищет по песням, альбомам и артистам одновременно.
Результат содержит ID треков, которые можно использовать для воспроизведения.

### Воспроизведение

```bash
# Воспроизвести трек по ID
synoplayer play music_12345

# Воспроизвести трек по имени (поиск + выбор)
synoplayer play "Comfortably Numb"
```

Плеер воспроизводит один трек и завершается.
Для непрерывного воспроизведения используйте плейлисты или TUI-режим.

### Плейлисты

```bash
# Список всех плейлистов (personal + shared)
synoplayer playlists

# Содержимое плейлиста (по имени или ID)
synoplayer playlist show "My Favorites"
synoplayer playlist show playlist_personal_normal/38

# Воспроизвести плейлист
synoplayer playlist play "My Favorites"

# Воспроизвести с перемешиванием
synoplayer playlist play "My Favorites" --shuffle

# Начать с определённого трека (нумерация с 1)
synoplayer playlist play "My Favorites" --from 5

# Режим повтора
synoplayer playlist play "My Favorites" --repeat all     # повтор всего плейлиста
synoplayer playlist play "My Favorites" --repeat one     # повтор одного трека

# Комбинирование параметров
synoplayer playlist play "Rock Mix" --shuffle --repeat all

# Создать пустой плейлист
synoplayer playlist create "New Playlist"

# Удалить плейлист
synoplayer playlist delete "Old Playlist"

# Переименовать
synoplayer playlist rename "Old Name" "New Name"

# Добавить/удалить трек
synoplayer playlist add "My Favorites" music_12345
synoplayer playlist remove "My Favorites" music_12345
```

### Импорт плейлистов

```bash
# Импорт .m3u файла с NAS
synoplayer playlist import "/volume1/homes/user/playlists/Rock.m3u"

# Импорт с указанием имени
synoplayer playlist import "/volume1/music/playlist.m3u" --name "Imported Rock"
```

### Умные плейлисты

```bash
# Создать плейлист по фильтрам
synoplayer playlist smart "Best Rock" --genre "Rock" --min-rating 4

# Фильтр по артисту
synoplayer playlist smart "Floyd Collection" --artist "Pink Floyd"

# Фильтр по году с лимитом
synoplayer playlist smart "2020 Hits" --year 2020 --limit 50

# Комбинирование фильтров
synoplayer playlist smart "Top Jazz" --genre "Jazz" --min-rating 3 --limit 200
```

### Рейтинги

```bash
# Установить рейтинг (1-5)
synoplayer rate music_12345 5

# Очистить рейтинг
synoplayer rate music_12345 0
```

### Избранное

```bash
# Добавить в избранное
synoplayer favorite music_12345

# Убрать из избранного
synoplayer unfavorite music_12345

# Список избранного
synoplayer favorites
```

### Тексты песен

```bash
# Показать тексты для указанного трека
synoplayer lyrics music_12345
```

### Интернет-радио

```bash
# Список радиостанций
synoplayer radio list

# Воспроизвести радиостанцию
synoplayer radio play "Station Name"

# Добавить радиостанцию
synoplayer radio add "My Radio" "http://stream.example.com/radio.mp3"
```

### Загрузка треков

```bash
# Скачать трек (автоматическое имя в текущей директории)
synoplayer download music_12345

# Указать путь для сохранения
synoplayer download music_12345 --output ~/Music/song.mp3
```

### История воспроизведения

```bash
# Показать историю
synoplayer history

# Очистить историю
synoplayer history clear
```

### Кеш

```bash
# Статус кеша (размер, количество файлов)
synoplayer cache status

# Список закешированных треков
synoplayer cache list

# Предзагрузить плейлист в кеш
synoplayer cache preload "My Favorites"

# Очистить весь кеш
synoplayer cache clear

# Удалить файлы старше N дней
synoplayer cache clear --older 30d
```

Кеш автоматически:
- сохраняет треки при воспроизведении (`cache_on_play = true`)
- проверяет целостность файлов по SHA-256 (`verify_integrity = true`)
- удаляет устаревшие файлы по TTL при запуске
- вытесняет старые файлы при превышении лимита (LRU)

---

## TUI — интерактивный плеер

TUI-режим предоставляет полнофункциональный интерфейс с навигацией, очередью воспроизведения и прогресс-баром.

### Запуск

```bash
synoplayer tui
```

### Интерфейс

```
┌─ SynoPlayer ──────────────────────────────────────────────┐
│  Library │ Folders │ Playlists │ Queue                     │
├───────────────────────────────────────────────────────────-─┤
│  Artist          │ Title              │ Album      │ Dur   │
│  ─────────────── │ ────────────────── │ ────────── │ ───── │
│► Pink Floyd      │ Comfortably Numb   │ The Wall   │ 6:23  │
│  Metallica       │ Nothing Else..     │ Metallica  │ 6:28  │
│  ...             │                    │            │       │
├────────────────────────────────────────────────────────────┤
│ ▶ Pink Floyd - Comfortably Numb  The Wall  2:15/6:23 [S]  │
│ ████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   │
├────────────────────────────────────────────────────────────┤
│ ↑↓:Nav Enter:Play Space:Stop n/p:Next/Prev s:ON r:all ... │
└────────────────────────────────────────────────────────────┘
```

### Клавиши

| Клавиша      | Действие                                           |
|-------------|-----------------------------------------------------|
| `↑` / `k`   | Курсор вверх                                       |
| `↓` / `j`   | Курсор вниз                                        |
| `PageUp`    | Страница вверх (10 строк)                           |
| `PageDown`  | Страница вниз (10 строк)                            |
| `Enter`     | Воспроизвести / открыть папку или плейлист          |
| `Space`     | Остановить воспроизведение                          |
| `n`         | Следующий трек в очереди                            |
| `p`         | Предыдущий трек в очереди                           |
| `s`         | Переключить shuffle (ON/off)                        |
| `r`         | Переключить repeat (off -> all -> one -> off)       |
| `Tab`       | Следующая вкладка                                   |
| `Shift+Tab` | Предыдущая вкладка                                  |
| `Esc`       | Назад (закрыть папку / детали плейлиста)            |
| `+` / `=`   | Увеличить громкость (+5%)                           |
| `-`         | Уменьшить громкость (-5%)                           |
| `q`         | Выход                                               |

### Вкладки

**Library** — полная библиотека песен с NAS. `Enter` начинает воспроизведение с выбранного трека. Все песни библиотеки добавляются в очередь.

**Folders** — навигация по файловой структуре NAS. Папки отмечены `[DIR]` и открываются через `Enter`. `Esc` — вверх на уровень. При выборе файла все файлы текущей папки добавляются в очередь.

**Playlists** — список всех плейлистов (personal + shared). `Enter` открывает содержимое плейлиста. Повторный `Enter` на треке начинает воспроизведение с полной очередью плейлиста. `Esc` — назад к списку.

**Queue** — текущая очередь воспроизведения. Текущий трек отмечен `▶` и выделен цветом.

### Режимы воспроизведения

**Shuffle** (`s`) — перемешивание. При включённом shuffle выбранный трек ставится первым, остальные перемешиваются. Индикатор `[S]` в строке плеера.

**Repeat** (`r`) — повтор:
- `off` — воспроизведение до конца очереди, затем стоп
- `all` — зацикливание всей очереди (индикатор `[R:*]`)
- `one` — повтор текущего трека (индикатор `[R:1]`)

### Прогресс-бар

В нижней части экрана:
- Текущий трек (артист — название, альбом)
- Прошедшее / общее время
- Индикаторы shuffle и repeat
- Визуальный прогресс-бар

При завершении трека автоматический переход к следующему (с учётом режима repeat).

---

## Конфигурация

Файл: `~/.config/synoplayer/config.toml`

```toml
[server]
host = "192.168.1.100"    # адрес NAS
port = 5001               # порт (5001 для HTTPS, 5000 для HTTP)
https = true              # использовать HTTPS
verify_ssl = true         # проверять SSL-сертификат

[auth]
username = "user"
credential_store = "keyring"  # "keyring" — OS keychain, "encrypted_file" — файл

[player]
default_volume = 80       # громкость по умолчанию (0-100)
buffer_size_kb = 256      # размер буфера

[cache]
enabled = true
path = "~/.cache/synoplayer/audio"
max_size_mb = 2048        # максимальный размер кеша
ttl_days = 30             # время жизни файлов в кеше
cache_on_play = true      # кешировать треки при воспроизведении
verify_integrity = true   # проверять SHA-256 хеш при чтении из кеша

[display]
show_lyrics = false
show_cover = false
```

---

## Структура проекта

```
├── Cargo.toml
├── CONVENTIONS.md                   # Конвенции разработки
├── AGENTS.md                   # Руководство по этапам
├── README.md
├── docs/
│   ├── API_REFERENCE.md        # Справочник Synology API
│   ├── SPECIFICATION.md        # Спецификация
│   └── *.pdf                   # Документация Synology
├── src/
│   ├── main.rs                 # Entry point, CLI dispatch
│   ├── lib.rs                  # Публичные модули
│   ├── playback.rs             # Общие функции воспроизведения
│   ├── history.rs              # История воспроизведения
│   ├── error.rs                # Типы ошибок
│   ├── api/                    # Synology API client
│   │   ├── client.rs           # HTTP transport, сессии
│   │   ├── auth.rs             # Login / logout / API discovery
│   │   ├── song.rs             # Песни (list, search, getinfo, rating)
│   │   ├── album.rs            # Альбомы
│   │   ├── artist.rs           # Артисты
│   │   ├── playlist.rs         # Плейлисты (CRUD, smart)
│   │   ├── stream.rs           # Стриминг аудио
│   │   ├── search.rs           # Поиск
│   │   ├── pin.rs              # Избранное
│   │   ├── cover.rs            # Обложки
│   │   ├── lyrics.rs           # Тексты
│   │   ├── radio.rs            # Интернет-радио
│   │   └── types.rs            # Типы данных API
│   ├── player/                 # Воспроизведение
│   │   ├── engine.rs           # Subprocess (ffplay/mpv/pw-play/paplay)
│   │   ├── state.rs            # State machine (Playing/Paused/Stopped)
│   │   └── queue.rs            # Очередь, shuffle, repeat
│   ├── tui/                    # Интерактивный TUI
│   │   ├── mod.rs              # Event loop, инициализация
│   │   ├── app.rs              # Состояние приложения
│   │   ├── ui.rs               # Виджеты и отрисовка
│   │   └── handler.rs          # Обработка клавиш
│   ├── cache/                  # Дисковый кеш
│   │   ├── manager.rs          # LRU, TTL, integrity
│   │   └── storage.rs          # I/O, SHA-256
│   ├── config/
│   │   └── model.rs            # TOML конфигурация
│   └── credentials/
│       └── store.rs            # Keyring / encrypted file
└── tests/
    ├── api_parsing.rs          # Тесты парсинга API
    ├── cli_integration.rs      # Тесты CLI
    └── fixtures/               # JSON-фикстуры Synology API
```

## Технологии

- **Rust** (edition 2024) — основной язык
- **tokio** — async runtime
- **reqwest** — HTTP client (cookie support)
- **clap** — CLI framework (derive)
- **ratatui** + **crossterm** — TUI-интерфейс
- **rodio** + **symphonia** — аудио (опционально)
- **serde** + **serde_json** — сериализация
- **toml** — конфигурация
- **tracing** — логирование
- **thiserror** + **anyhow** — обработка ошибок
- **keyring** — хранение учётных данных (OS keychain)
- **sha2** — целостность кеша

## Этапы разработки

0. **Подготовка** — спецификация, требования, документация API
1. **Фундамент** — скелет проекта, HTTP клиент, аутентификация
2. **MVP** — стриминг и воспроизведение
3. **Библиотека** — навигация по коллекции (альбомы, артисты, жанры, папки)
4. **Плейлисты** — полное управление (CRUD, import, smart)
5. **Рейтинги и избранное** — pin/unpin, rating 1-5
6. **Доп. функции** — lyrics, radio, download, history, cache
7. **Кеш** — дисковый LRU-кеш с SHA-256, TTL, preload
8. **TUI** — интерактивный плеер (ratatui)

Подробности в [AGENTS.md](AGENTS.md) и [docs/SPECIFICATION.md](docs/SPECIFICATION.md).
