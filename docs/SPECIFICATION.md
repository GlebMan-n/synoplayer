# SynoPlayer — Спецификация разработки

## 1. Описание продукта

**SynoPlayer** — CLI аудиоплеер для Synology Audio Station.

### Пользовательские сценарии
1. Разработчик/сисадмин хочет слушать музыку с NAS из терминала
2. Запуск на headless одноплатнике как часть умной колонки
3. Автоматизация через скрипты (cron: утренний плейлист, и т.д.)

### Платформы
- Linux x86_64 (основная разработка)
- Linux aarch64 (одноплатник: Raspberry Pi, Orange Pi, и т.д.)
- macOS (вторично)

---

## 2. Выбор технологий

### Почему Rust

| Критерий | Rust | Python | Go |
|----------|------|--------|----|
| Размер бинарника | ~5-10 MB | ~50+ MB (venv) | ~10-15 MB |
| Потребление RAM | ~5-15 MB | ~30-50 MB | ~10-20 MB |
| Cross-compilation ARM | Тривиально | Сложно (native deps) | Тривиально |
| Аудио библиотеки | rodio/symphonia | pyaudio/pygame | Слабо |
| CLI framework | clap (отлично) | click (отлично) | cobra (хорошо) |
| Async HTTP | reqwest (отлично) | aiohttp (отлично) | net/http (хорошо) |
| TUI | ratatui (отлично) | textual (хорошо) | bubbletea (отлично) |
| Безопасность памяти | Гарантирована | N/A | GC |

**Вывод**: Rust оптимален для CLI-утилиты, которая будет работать на SBC с ограниченными ресурсами.

### Ключевые зависимости (Cargo.toml)

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "cookies", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
clap = { version = "4", features = ["derive"] }
rodio = "0.20"
symphonia = { version = "0.5", features = ["all"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
thiserror = "2"
anyhow = "1"
dirs = "6"
keyring = "3"
sha2 = "0.10"
aes-gcm = "0.10"              # fallback credential encryption
```

---

## 3. Архитектура приложения

### Структура модулей

```
synoplayer/
├── Cargo.toml
├── CONVENTIONS.md
├── AGENTS.md
├── README.md
├── docs/
│   ├── API_REFERENCE.md
│   ├── SPECIFICATION.md
│   ├── AS_Guide.pdf
│   └── DSM_Developer_Guide_7_enu.pdf
├── src/
│   ├── main.rs
│   ├── error.rs
│   ├── cli/
│   │   ├── mod.rs          # Cli struct, subcommands enum
│   │   ├── play.rs         # play, pause, stop, next, prev, volume
│   │   ├── playlist.rs     # playlist CRUD
│   │   ├── library.rs      # songs, albums, artists, genres, folders
│   │   ├── search.rs       # search
│   │   └── config.rs       # config management
│   ├── api/
│   │   ├── mod.rs
│   │   ├── client.rs       # SynoClient: base HTTP, auto-relogin
│   │   ├── auth.rs         # Login/logout
│   │   ├── song.rs
│   │   ├── album.rs
│   │   ├── artist.rs
│   │   ├── playlist.rs
│   │   ├── stream.rs
│   │   ├── search.rs
│   │   ├── pin.rs
│   │   ├── cover.rs
│   │   ├── lyrics.rs
│   │   ├── radio.rs
│   │   ├── folder.rs
│   │   ├── genre.rs
│   │   ├── composer.rs
│   │   └── types.rs        # Song, Album, Artist, etc.
│   ├── player/
│   │   ├── mod.rs
│   │   ├── engine.rs       # AudioEngine: rodio Sink management
│   │   ├── queue.rs        # PlayQueue: ordered list + shuffle
│   │   └── state.rs        # PlayerState enum + current track info
│   ├── cache/
│   │   ├── mod.rs
│   │   ├── manager.rs      # CacheManager: LRU eviction, TTL cleanup
│   │   └── storage.rs      # Disk I/O, hash verification
│   ├── credentials/
│   │   ├── mod.rs
│   │   └── store.rs        # CredentialStore: keyring + encrypted file fallback
│   └── config/
│       ├── mod.rs
│       └── model.rs        # ServerConfig, AuthConfig, CacheConfig
└── tests/
    ├── api_mock.rs         # Mock HTTP tests for API client
    └── integration.rs      # Integration tests (requires real NAS)
```

### Поток данных

```
User Input (CLI)
    → Parse (clap)
    → Command Handler
    → API Client (reqwest → Synology NAS)
    → Response Processing
    → Player Engine (rodio) или Console Output
```

### Конфигурация

Файл: `~/.config/synoplayer/config.toml`

```toml
[server]
host = "192.168.1.100"
port = 5001
https = true
verify_ssl = false          # для self-signed сертификатов
quickconnect_id = ""        # опционально

[auth]
username = "user"
# пароль хранится в OS keyring или зашифрованном файле
# после первого `synoplayer login` — подключается автоматически
credential_store = "keyring"  # "keyring" | "encrypted_file"

[player]
default_volume = 80
output_device = ""          # default system output
buffer_size_kb = 256

[cache]
enabled = true
path = "~/.cache/synoplayer/audio"
max_size_mb = 2048            # 2 GB, LRU-вытеснение при превышении
ttl_days = 30                 # автоудаление файлов старше N дней
cache_on_play = true          # кешировать при воспроизведении
preload_playlist = false      # предзагрузка всех треков плейлиста
transcode_before_cache = false # кешировать оригинал или транскод
verify_integrity = true       # проверка SHA-256 при чтении из кеша

[display]
show_lyrics = false
show_cover = false          # для терминалов с sixel
```

Сессия: `~/.config/synoplayer/session.json`

```json
{
  "sid": "abc123...",
  "created_at": "2026-03-17T10:00:00Z",
  "server": "192.168.1.100:5001"
}
```

---

## 4. API Client Design

### SynoClient

```rust
pub struct SynoClient {
    http: reqwest::Client,
    base_url: String,
    sid: Option<String>,
    api_paths: HashMap<String, ApiInfo>,  // кеш из SYNO.API.Info
}
```

**Ключевые методы:**
- `new(config) → SynoClient` — создать клиент
- `login(username, password) → Result<()>` — аутентификация
- `logout() → Result<()>` — завершение сессии
- `request(api, version, method, params) → Result<Value>` — универсальный вызов
- `discover_apis() → Result<()>` — заполнить api_paths

**Auto-relogin**: При получении error code 106 или 119 — автоматическая повторная аутентификация и retry запроса.

---

## 5. Player Engine Design

### AudioEngine

```rust
pub struct AudioEngine {
    sink: rodio::Sink,
    stream_handle: rodio::OutputStreamHandle,
    state: PlayerState,
    volume: f32,
}

pub enum PlayerState {
    Stopped,
    Playing { track: TrackInfo, position: Duration },
    Paused { track: TrackInfo, position: Duration },
}
```

**Стриминг**: HTTP GET → bytes stream → symphonia decoder → rodio Sink.

Буферизация: скачиваем чанками (256KB default), подаём в decoder по мере готовности.

---

## 6. Credential Store Design

### CredentialStore

```rust
pub enum CredentialBackend {
    Keyring,        // OS keyring (libsecret / Keychain / Credential Manager)
    EncryptedFile,  // AES-256-GCM, key derived from machine-id
}

pub struct CredentialStore {
    backend: CredentialBackend,
}

impl CredentialStore {
    pub fn save(username: &str, password: &str) -> Result<()>;
    pub fn load() -> Result<Option<(String, String)>>;
    pub fn clear() -> Result<()>;
    pub fn exists() -> bool;
}
```

**Поток автоподключения:**
1. Пользователь запускает любую команду (напр. `synoplayer songs`)
2. Нет активной сессии → проверяем `CredentialStore::load()`
3. Есть сохранённые credentials → автоматический `login()`
4. Нет credentials → ошибка "Run `synoplayer login` first"

**Keyring backend** (по умолчанию):
- Linux: D-Bus Secret Service (GNOME Keyring, KWallet)
- macOS: Keychain
- Crate: `keyring` v3
- Service name: `synoplayer`, user: `<username>@<host>:<port>`

**Encrypted file backend** (fallback для headless/SBC):
- Файл: `~/.config/synoplayer/credentials.enc`
- Шифрование: AES-256-GCM
- Ключ: PBKDF2 от machine-id (`/etc/machine-id` или `/var/lib/dbus/machine-id`)
- Не является криптографически стойким (machine-id известен локально), но защищает от случайного чтения

---

## 7. Audio Cache Design

### CacheManager

```rust
pub struct CacheConfig {
    pub enabled: bool,
    pub path: PathBuf,
    pub max_size_mb: u64,
    pub ttl_days: u32,
    pub cache_on_play: bool,
    pub preload_playlist: bool,
    pub transcode_before_cache: bool,
    pub verify_integrity: bool,
}

pub struct CacheManager {
    config: CacheConfig,
    index: CacheIndex,  // in-memory index, persisted to index.json
}

pub struct CacheEntry {
    pub song_id: String,
    pub file_path: PathBuf,
    pub size_bytes: u64,
    pub sha256: String,
    pub cached_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
    pub format: String,         // "flac", "mp3", etc.
    pub metadata: SongMetadata, // title, artist, album
}
```

**Структура на диске:**
```
~/.cache/synoplayer/audio/
├── index.json              # CacheIndex: список всех записей
├── a1b2c3d4.audio          # аудиофайл (имя = SHA-256 от song_id)
├── a1b2c3d4.meta           # метаданные (JSON)
├── e5f6g7h8.audio
├── e5f6g7h8.meta
└── ...
```

**LRU-вытеснение:**
1. При добавлении нового файла → проверить total size
2. Если total > max_size_mb → удалять файлы с самым старым `last_accessed`
3. Удалять пока total < max_size_mb * 0.9 (10% гистерезис)

**TTL-очистка:**
- При запуске приложения и каждые 24 часа (в daemon mode)
- Удалять записи с `cached_at` старше `ttl_days`

**Поток воспроизведения с кешем:**
```
play(song_id)
  → cache.get(song_id)
    → HIT + verify_integrity OK → play from disk
    → HIT + verify_integrity FAIL → delete, re-download
    → MISS → stream from server
      → if cache_on_play → tee: decode + save to cache simultaneously
```

---

## 8. Тестирование (TDD)

### Философия: Tests First

Разработка ведётся в стиле TDD — **тесты пишутся до реализации**.
Это возможно потому что у нас есть чёткие контракты:
- Формат запросов/ответов Synology API (документирован в API_REFERENCE.md)
- State machine плеера (конечный набор состояний и переходов)
- Поведение кеша (LRU, TTL, integrity — чистая логика)
- Формат конфигурации (TOML-схема определена)

### Порядок: Этап 0 → Тесты → Этап 1 → Реализация

**Этап 0 (до реализации)** создаёт:
1. Cargo workspace со всеми зависимостями (включая dev-dependencies)
2. Структуру модулей (пустые файлы с pub trait/struct заглушками)
3. Все тесты (помечены `#[ignore]` до реализации)
4. Тестовые fixtures (JSON-файлы с ответами API)

По мере реализации каждого модуля:
1. Снять `#[ignore]` с соответствующих тестов
2. Реализовать код до прохождения тестов
3. Рефакторинг при зелёных тестах

### Dev-зависимости

```toml
[dev-dependencies]
wiremock = "0.6"              # mock HTTP server
tokio-test = "0.4"            # async test utilities
tempfile = "3"                # temporary dirs for cache tests
assert_fs = "1"               # filesystem assertions
predicates = "3"              # flexible assertions
assert_cmd = "2"              # CLI integration tests
serde_json = "1"              # fixture loading (also in deps)
```

### Структура тестов

```
src/
├── api/
│   ├── client.rs             # #[cfg(test)] mod tests — inline
│   ├── auth.rs               # #[cfg(test)] mod tests
│   ├── song.rs               # #[cfg(test)] mod tests
│   └── ...                   # каждый модуль с inline тестами
├── player/
│   ├── queue.rs              # #[cfg(test)] mod tests
│   └── state.rs              # #[cfg(test)] mod tests
├── cache/
│   ├── manager.rs            # #[cfg(test)] mod tests
│   └── storage.rs            # #[cfg(test)] mod tests
├── credentials/
│   └── store.rs              # #[cfg(test)] mod tests
└── config/
    └── model.rs              # #[cfg(test)] mod tests

tests/                        # интеграционные тесты
├── fixtures/                 # JSON-ответы Synology API
│   ├── api_info_response.json
│   ├── auth_login_success.json
│   ├── auth_login_2fa_required.json
│   ├── auth_login_wrong_password.json
│   ├── song_list_response.json
│   ├── song_search_response.json
│   ├── album_list_response.json
│   ├── artist_list_response.json
│   ├── playlist_list_response.json
│   ├── playlist_getinfo_response.json
│   ├── stream_headers.json
│   ├── pin_list_response.json
│   ├── error_session_expired.json
│   └── error_no_permission.json
├── api_integration.rs        # тесты с реальным NAS (env-gated)
├── cli_integration.rs        # тесты CLI через assert_cmd
└── cache_integration.rs      # тесты кеша на реальной FS

```

### Категории тестов

#### 1. API Response Parsing (unit, пишутся первыми)

Загружаем fixture JSON → парсим в типы → проверяем поля.
Не требуют сети, не требуют реализации HTTP-клиента.

```rust
// Пример: tests пишутся ДО реализации Song::from_json()
#[test]
#[ignore] // снять когда Song будет реализован
fn parse_song_list_response() {
    let json = include_str!("../tests/fixtures/song_list_response.json");
    let response: ApiResponse<SongListData> = serde_json::from_str(json).unwrap();

    assert!(response.success);
    assert_eq!(response.data.total, 1234);
    assert!(!response.data.songs.is_empty());

    let song = &response.data.songs[0];
    assert!(!song.id.is_empty());
    assert!(!song.title.is_empty());
}

#[test]
#[ignore]
fn parse_song_with_rating() {
    let json = include_str!("../tests/fixtures/song_list_response.json");
    let response: ApiResponse<SongListData> = serde_json::from_str(json).unwrap();
    let song = &response.data.songs[0];

    assert!(song.additional.rating >= 0 && song.additional.rating <= 5);
}

#[test]
#[ignore]
fn parse_error_session_expired() {
    let json = include_str!("../tests/fixtures/error_session_expired.json");
    let response: ApiResponse<()> = serde_json::from_str(json).unwrap();

    assert!(!response.success);
    assert_eq!(response.error.unwrap().code, 106);
}
```

#### 2. API Client (unit + wiremock)

Mock HTTP server → проверяем что клиент формирует правильные запросы
и обрабатывает ответы.

```rust
#[tokio::test]
#[ignore]
async fn login_sends_correct_params() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(query_param("api", "SYNO.API.Auth"))
        .and(query_param("method", "login"))
        .and(query_param("account", "testuser"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(json!({"success": true, "data": {"sid": "test_sid"}})))
        .mount(&server).await;

    let client = SynoClient::new(&server.uri());
    client.login("testuser", "testpass").await.unwrap();
    assert!(client.is_authenticated());
}

#[tokio::test]
#[ignore]
async fn auto_relogin_on_session_expired() {
    // Первый запрос → 106 (expired)
    // Автоматический re-login
    // Повтор запроса → успех
}

#[tokio::test]
#[ignore]
async fn api_discovery_populates_paths() {
    // Mock SYNO.API.Info → проверить что пути закешировались
}
```

#### 3. Player State Machine (unit, чистая логика)

Не требуют аудио. Тестируют переходы состояний.

```rust
#[test]
#[ignore]
fn stopped_to_playing() {
    let mut state = PlayerState::Stopped;
    state.play(track_info());
    assert!(matches!(state, PlayerState::Playing { .. }));
}

#[test]
#[ignore]
fn playing_to_paused() {
    let mut state = PlayerState::playing(track_info());
    state.pause();
    assert!(matches!(state, PlayerState::Paused { .. }));
}

#[test]
#[ignore]
fn paused_to_playing_resumes() {
    let mut state = PlayerState::paused(track_info(), Duration::from_secs(30));
    state.resume();
    match &state {
        PlayerState::Playing { position, .. } => assert_eq!(*position, Duration::from_secs(30)),
        _ => panic!("expected Playing"),
    }
}

#[test]
#[ignore]
fn stop_from_any_state() {
    for initial in [PlayerState::playing(track_info()), PlayerState::paused(track_info(), Duration::ZERO)] {
        let mut state = initial;
        state.stop();
        assert!(matches!(state, PlayerState::Stopped));
    }
}
```

#### 4. Play Queue (unit, чистая логика)

```rust
#[test]
#[ignore]
fn queue_add_and_next() {
    let mut q = PlayQueue::new();
    q.add(song("a"));
    q.add(song("b"));
    q.add(song("c"));
    assert_eq!(q.current().unwrap().id, "a");
    q.next();
    assert_eq!(q.current().unwrap().id, "b");
}

#[test]
#[ignore]
fn queue_prev_at_start_stays() {
    let mut q = PlayQueue::from(vec![song("a"), song("b")]);
    assert_eq!(q.current().unwrap().id, "a");
    q.prev(); // уже в начале
    assert_eq!(q.current().unwrap().id, "a");
}

#[test]
#[ignore]
fn queue_shuffle_changes_order() {
    let mut q = PlayQueue::from(vec![song("a"), song("b"), song("c"), song("d"), song("e")]);
    let original: Vec<_> = q.list().iter().map(|s| s.id.clone()).collect();
    q.shuffle();
    let shuffled: Vec<_> = q.list().iter().map(|s| s.id.clone()).collect();
    // тот же набор, но (скорее всего) другой порядок
    assert_eq!(original.len(), shuffled.len());
    // все элементы на месте
    for id in &original {
        assert!(shuffled.contains(id));
    }
}

#[test]
#[ignore]
fn queue_repeat_one_stays_on_track() {
    let mut q = PlayQueue::from(vec![song("a"), song("b")]);
    q.set_repeat(RepeatMode::One);
    q.next();
    assert_eq!(q.current().unwrap().id, "a"); // не сдвинулось
}

#[test]
#[ignore]
fn queue_repeat_all_wraps_around() {
    let mut q = PlayQueue::from(vec![song("a"), song("b")]);
    q.set_repeat(RepeatMode::All);
    q.next(); // b
    q.next(); // wrap → a
    assert_eq!(q.current().unwrap().id, "a");
}
```

#### 5. Cache Manager (unit, tempdir)

```rust
#[test]
#[ignore]
fn cache_store_and_retrieve() {
    let dir = tempfile::tempdir().unwrap();
    let cache = CacheManager::new(CacheConfig { path: dir.path().into(), max_size_mb: 100, ..default() });

    cache.put("song_1", b"fake audio data", &metadata()).unwrap();
    assert!(cache.contains("song_1"));

    let data = cache.get("song_1").unwrap().unwrap();
    assert_eq!(data, b"fake audio data");
}

#[test]
#[ignore]
fn cache_lru_eviction() {
    let dir = tempfile::tempdir().unwrap();
    let cache = CacheManager::new(CacheConfig {
        path: dir.path().into(),
        max_size_mb: 1, // 1 MB
        ..default()
    });

    // Заполнить кеш файлами по 300KB (3 файла = 900KB < 1MB)
    cache.put("song_1", &vec![0u8; 300_000], &metadata()).unwrap();
    cache.put("song_2", &vec![0u8; 300_000], &metadata()).unwrap();
    cache.put("song_3", &vec![0u8; 300_000], &metadata()).unwrap();
    assert!(cache.contains("song_1"));

    // Добавить ещё 300KB → превышение → song_1 (oldest) должен быть вытеснен
    cache.put("song_4", &vec![0u8; 300_000], &metadata()).unwrap();
    assert!(!cache.contains("song_1")); // вытеснен
    assert!(cache.contains("song_4"));  // добавлен
}

#[test]
#[ignore]
fn cache_ttl_expiration() {
    let dir = tempfile::tempdir().unwrap();
    let cache = CacheManager::new(CacheConfig {
        path: dir.path().into(),
        ttl_days: 0, // мгновенная протухаемость
        ..default()
    });

    cache.put("song_1", b"data", &metadata()).unwrap();
    cache.cleanup_expired().unwrap();
    assert!(!cache.contains("song_1"));
}

#[test]
#[ignore]
fn cache_integrity_check_detects_corruption() {
    let dir = tempfile::tempdir().unwrap();
    let cache = CacheManager::new(CacheConfig {
        path: dir.path().into(),
        verify_integrity: true,
        ..default()
    });

    cache.put("song_1", b"original data", &metadata()).unwrap();

    // Испортить файл на диске
    let file_path = cache.file_path("song_1");
    std::fs::write(&file_path, b"corrupted!").unwrap();

    // get должен вернуть None (или Err) и удалить повреждённый файл
    assert!(cache.get("song_1").unwrap().is_none());
    assert!(!cache.contains("song_1"));
}

#[test]
#[ignore]
fn cache_status_reports_correct_stats() {
    let dir = tempfile::tempdir().unwrap();
    let cache = CacheManager::new(CacheConfig { path: dir.path().into(), max_size_mb: 100, ..default() });

    cache.put("s1", &vec![0u8; 1000], &metadata()).unwrap();
    cache.put("s2", &vec![0u8; 2000], &metadata()).unwrap();

    let status = cache.status().unwrap();
    assert_eq!(status.file_count, 2);
    assert_eq!(status.total_size_bytes, 3000);
    assert_eq!(status.max_size_bytes, 100 * 1024 * 1024);
}

#[test]
#[ignore]
fn cache_disabled_does_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let cache = CacheManager::new(CacheConfig { enabled: false, path: dir.path().into(), ..default() });

    cache.put("song_1", b"data", &metadata()).unwrap();
    assert!(!cache.contains("song_1")); // не сохранился
}
```

#### 6. Credentials Store (unit, tempdir)

```rust
#[test]
#[ignore]
fn encrypted_file_store_save_and_load() {
    let dir = tempfile::tempdir().unwrap();
    let store = CredentialStore::encrypted_file(dir.path().join("creds.enc"));

    store.save("myuser", "mypass").unwrap();
    let (user, pass) = store.load().unwrap().unwrap();
    assert_eq!(user, "myuser");
    assert_eq!(pass, "mypass");
}

#[test]
#[ignore]
fn encrypted_file_store_clear() {
    let dir = tempfile::tempdir().unwrap();
    let store = CredentialStore::encrypted_file(dir.path().join("creds.enc"));

    store.save("user", "pass").unwrap();
    store.clear().unwrap();
    assert!(store.load().unwrap().is_none());
}

#[test]
#[ignore]
fn encrypted_file_store_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let store = CredentialStore::encrypted_file(dir.path().join("creds.enc"));

    store.save("user1", "pass1").unwrap();
    store.save("user2", "pass2").unwrap();
    let (user, pass) = store.load().unwrap().unwrap();
    assert_eq!(user, "user2");
    assert_eq!(pass, "pass2");
}

#[test]
#[ignore]
fn load_from_empty_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let store = CredentialStore::encrypted_file(dir.path().join("creds.enc"));
    assert!(store.load().unwrap().is_none());
}
```

#### 7. Config (unit)

```rust
#[test]
#[ignore]
fn parse_full_config() {
    let toml = r#"
    [server]
    host = "192.168.1.100"
    port = 5001
    https = true
    verify_ssl = false

    [auth]
    username = "admin"
    credential_store = "keyring"

    [player]
    default_volume = 75
    buffer_size_kb = 512

    [cache]
    enabled = true
    path = "/tmp/test_cache"
    max_size_mb = 1024
    ttl_days = 14
    cache_on_play = true
    preload_playlist = false
    transcode_before_cache = false
    verify_integrity = true
    "#;

    let config: AppConfig = toml::from_str(toml).unwrap();
    assert_eq!(config.server.host, "192.168.1.100");
    assert_eq!(config.server.port, 5001);
    assert!(!config.server.verify_ssl);
    assert_eq!(config.cache.max_size_mb, 1024);
    assert_eq!(config.player.default_volume, 75);
}

#[test]
#[ignore]
fn config_defaults_when_optional_missing() {
    let toml = r#"
    [server]
    host = "10.0.0.1"
    "#;

    let config: AppConfig = toml::from_str(toml).unwrap();
    assert_eq!(config.server.port, 5001);     // default
    assert!(config.server.https);              // default
    assert!(config.cache.enabled);             // default
    assert_eq!(config.cache.max_size_mb, 2048); // default
    assert_eq!(config.player.default_volume, 80); // default
}

#[test]
#[ignore]
fn config_serialize_roundtrip() {
    let config = AppConfig::default();
    let serialized = toml::to_string(&config).unwrap();
    let deserialized: AppConfig = toml::from_str(&serialized).unwrap();
    assert_eq!(config, deserialized);
}
```

#### 8. CLI Integration (assert_cmd)

```rust
#[test]
#[ignore]
fn cli_help_shows_usage() {
    Command::cargo_bin("synoplayer").unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Usage"));
}

#[test]
#[ignore]
fn cli_no_config_shows_error() {
    Command::cargo_bin("synoplayer").unwrap()
        .env("HOME", tempfile::tempdir().unwrap().path())
        .arg("songs")
        .assert()
        .failure()
        .stderr(predicates::str::contains("config"));
}

#[test]
#[ignore]
fn cli_cache_status_outputs_stats() {
    Command::cargo_bin("synoplayer").unwrap()
        .arg("cache")
        .arg("status")
        .assert()
        .success()
        .stdout(predicates::str::contains("files"))
        .stdout(predicates::str::contains("MB"));
}
```

#### 9. Integration-тесты с реальным NAS

Запускаются только при наличии env vars. Не запускаются в CI.

```rust
fn nas_config() -> Option<(String, String, String)> {
    let host = std::env::var("SYNO_HOST").ok()?;
    let user = std::env::var("SYNO_USER").ok()?;
    let pass = std::env::var("SYNO_PASS").ok()?;
    Some((host, user, pass))
}

#[tokio::test]
#[ignore]
async fn integration_login_logout() {
    let Some((host, user, pass)) = nas_config() else { return };
    let client = SynoClient::new(&host);
    client.login(&user, &pass).await.unwrap();
    assert!(client.is_authenticated());
    client.logout().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn integration_list_songs() {
    let Some((host, user, pass)) = nas_config() else { return };
    let client = SynoClient::new(&host);
    client.login(&user, &pass).await.unwrap();
    let songs = client.songs().list(0, 10).await.unwrap();
    assert!(!songs.is_empty());
}
```

### Запуск тестов

```bash
# Все unit-тесты (быстрые, без сети)
cargo test

# Включая ignored (реализованные, но помеченные)
cargo test -- --ignored

# Только конкретный модуль
cargo test api::auth
cargo test cache::manager
cargo test player::queue

# Integration с NAS
SYNO_HOST=https://192.168.1.100:5001 SYNO_USER=admin SYNO_PASS=secret cargo test integration_ -- --ignored

# С логами
RUST_LOG=debug cargo test -- --nocapture
```

### Метрики покрытия

Целевое покрытие тестами:
- `api/types.rs` (parsing): **100%** — все поля, все варианты ответов
- `api/client.rs` (HTTP logic): **90%+** — все методы, error paths, retry
- `player/queue.rs`: **100%** — все операции, edge cases
- `player/state.rs`: **100%** — все переходы state machine
- `cache/manager.rs`: **95%+** — LRU, TTL, integrity, disabled mode
- `credentials/store.rs`: **95%+** — save/load/clear, оба бэкенда
- `config/model.rs`: **100%** — parsing, defaults, roundtrip

Инструмент: `cargo-tarpaulin` или `cargo-llvm-cov`.

---

## 9. CI/CD (будущее)

```yaml
# .github/workflows/ci.yml
- cargo fmt --check
- cargo clippy -- -D warnings
- cargo test
- cargo build --release --target x86_64-unknown-linux-gnu
- cargo build --release --target aarch64-unknown-linux-gnu  # cross
```

---

## 10. Метрики готовности MVP

MVP считается готовым когда:
- [ ] Можно подключиться к NAS и залогиниться
- [ ] Credentials сохраняются, повторный login не нужен
- [ ] Можно просмотреть список песен
- [ ] Можно найти песню по имени
- [ ] Можно воспроизвести песню (аудио играет в динамиках)
- [ ] Работают play/pause/stop/next/prev
- [ ] Отображается текущий трек
- [ ] Работает управление громкостью
