# Synology Audio Station API Reference

Справочник по Web API Synology Audio Station для разработки SynoPlayer.

> **Важно**: Официальной документации API нет. Информация собрана из community-источников,
> open-source клиентов и reverse-engineering.

---

## Общие принципы

### Base URL
```
https://<host>:5001/webapi/<path>
http://<host>:5000/webapi/<path>
```

### Формат запроса
Все вызовы — HTTP GET или POST с URL-encoded параметрами:

| Параметр | Описание |
|----------|----------|
| `api` | Имя API, напр. `SYNO.AudioStation.Song` |
| `version` | Версия API (integer) |
| `method` | Вызываемый метод |
| `_sid` | Session ID (из login) |

### Формат ответа

**Успех:**
```json
{"success": true, "data": { ... }}
```

**Ошибка:**
```json
{"success": false, "error": {"code": 100}}
```

### Коды ошибок

| Код | Значение |
|-----|----------|
| 100 | Неизвестная ошибка |
| 101 | Не указан api/method/version |
| 102 | API не существует |
| 103 | Метод не существует |
| 104 | Версия не поддерживается |
| 105 | Нет прав доступа |
| 106 | Таймаут сессии |
| 107 | Сессия прервана (дублирующий логин) |
| 119 | Невалидная сессия |

### Пагинация и сортировка
Большинство `list`-методов поддерживают:

| Параметр | Описание |
|----------|----------|
| `offset` | Начальный индекс (0-based) |
| `limit` | Макс. количество записей |
| `sort_by` | Поле сортировки |
| `sort_direction` | `asc` / `desc` |
| `library` | Фильтр: `shared`, `personal` |

### Параметр `additional`
Запрашивает дополнительные метаданные (через запятую):
- `song_tag` — title, album, artist, album_artist, composer, comment, disc, track, year, genre
- `song_audio` — duration, bitrate, codec, container, frequency, channel, lossless, filesize
- `song_rating` — rating (0-5)

---

## 1. SYNO.API.Info

**Path**: `/webapi/query.cgi`
**Version**: 1

### query
Обнаружение всех доступных API.

```
GET /webapi/query.cgi?api=SYNO.API.Info&version=1&method=query&query=all
```

**Ответ**: Список API с `path`, `minVersion`, `maxVersion`.

---

## 2. SYNO.API.Auth

**Path**: `/webapi/entry.cgi`
**Versions**: 1-7

### login

| Параметр | Обязателен | Описание |
|----------|------------|----------|
| `account` | Да | Имя пользователя |
| `passwd` | Да | Пароль |
| `session` | Нет | Имя сессии (напр. `AudioStation`) |
| `format` | Нет | `cookie` (default) или `sid` |
| `otp_code` | Нет | TOTP код для 2FA |
| `enable_device_token` | Нет | `yes` для "запомнить устройство" |
| `device_name` | Нет | Имя устройства |
| `device_id` | Нет | Ранее полученный device token |

**Ответ**:
```json
{"success": true, "data": {"sid": "abc123..."}}
```

### logout

| Параметр | Описание |
|----------|----------|
| `session` | Имя сессии |

---

## 3. SYNO.AudioStation.Info

**Path**: `AudioStation/info.cgi`
**Versions**: 1-4

### getinfo
Возвращает версию Audio Station и DSM.

---

## 4. SYNO.AudioStation.Song

**Path**: `AudioStation/song.cgi`
**Versions**: 1-3

### list

| Параметр | Описание |
|----------|----------|
| `offset` | Начальный индекс |
| `limit` | Макс. количество |
| `album` | Фильтр по альбому |
| `artist` | Фильтр по артисту |
| `album_artist` | Фильтр по album_artist |
| `composer` | Фильтр по композитору |
| `genre` | Фильтр по жанру |
| `folder_id` | Фильтр по папке |
| `additional` | `song_tag,song_audio,song_rating` |

### search

| Параметр | Описание |
|----------|----------|
| `keyword` | Строка поиска |
| `offset`, `limit` | Пагинация |
| `additional` | Доп. метаданные |

### getinfo

| Параметр | Описание |
|----------|----------|
| `id` | ID песни (напр. `music_12345`) |
| `additional` | Доп. метаданные |

### setrating (v2+)

| Параметр | Описание |
|----------|----------|
| `id` | ID песни |
| `rating` | 0-5 (0 = без рейтинга) |

---

## 5. SYNO.AudioStation.Album

**Path**: `AudioStation/album.cgi`
**Versions**: 1-3

### list

| Параметр | Описание |
|----------|----------|
| `offset`, `limit` | Пагинация |
| `artist` | Фильтр по артисту |
| `album_artist` | Фильтр |
| `genre` | Фильтр по жанру |
| `keyword` | Поиск |

---

## 6. SYNO.AudioStation.Artist

**Path**: `AudioStation/artist.cgi`
**Versions**: 1-4

### list

| Параметр | Описание |
|----------|----------|
| `offset`, `limit` | Пагинация |
| `genre` | Фильтр по жанру |
| `keyword` | Поиск |

---

## 7. SYNO.AudioStation.Playlist

**Path**: `AudioStation/playlist.cgi`
**Versions**: 1-3

### list
Возвращает все плейлисты. Параметры: `offset`, `limit`, `library`.

### getinfo
| Параметр | Описание |
|----------|----------|
| `id` | ID плейлиста |

Возвращает плейлист со списком песен.

### create
| Параметр | Описание |
|----------|----------|
| `name` | Название |
| `library` | `shared` / `personal` |
| `songs` | Через запятую ID песен |

### delete
| Параметр | Описание |
|----------|----------|
| `id` | ID плейлиста |

### rename
| Параметр | Описание |
|----------|----------|
| `id` | ID плейлиста |
| `new_name` | Новое название |

### updatesongs
| Параметр | Описание |
|----------|----------|
| `id` | ID плейлиста |
| `songs` | Через запятую ID песен |
| `offset` | Позиция вставки |

### createsmart
Создание smart-плейлиста с правилами фильтрации (жанр, артист, рейтинг, год и т.д.).

---

## 8. SYNO.AudioStation.Stream

**Path**: `AudioStation/stream.cgi`
**Versions**: 1-2

### stream
Стриминг оригинального файла.
```
GET /webapi/AudioStation/stream.cgi/0.mp3?api=SYNO.AudioStation.Stream&version=2&method=stream&id=<song_id>&_sid=<sid>
```

Ответ: бинарный аудиопоток (audio/mpeg, audio/flac, etc.)

### transcode
Транскодирование в MP3 на лету.
```
GET /webapi/AudioStation/stream.cgi/0.mp3?api=SYNO.AudioStation.Stream&version=2&method=transcode&id=<song_id>&_sid=<sid>
```

Поддерживаемые входные форматы: AIF, AIFF, APE, ALAC, FLAC, M4A, M4B, MP3, WAV, OGG, WMA, DSD.

---

## 9. SYNO.AudioStation.Search

**Path**: `AudioStation/search.cgi`
**Version**: 1

### list
| Параметр | Описание |
|----------|----------|
| `keyword` | Строка поиска |
| `offset`, `limit` | Пагинация |
| `additional` | Доп. метаданные |

Возвращает `songs`, `albums`, `artists` массивы.

---

## 10. SYNO.AudioStation.Pin (Избранное)

**Path**: `AudioStation/pinlist.cgi`
**Version**: 1

### pin
Добавить в избранное.

### unpin
Удалить из избранного.

### list
Список избранного.

### rename, reorder
Управление отображением.

---

## 11. SYNO.AudioStation.Cover

**Path**: `AudioStation/cover.cgi`
**Versions**: 1-3

### getsongcover
| Параметр | Описание |
|----------|----------|
| `id` | ID песни |

Ответ: бинарная картинка (image/jpeg, image/png).

### getfoldercover
| Параметр | Описание |
|----------|----------|
| `id` | ID папки |

---

## 12. SYNO.AudioStation.Lyrics

**Path**: `AudioStation/lyrics.cgi`
**Versions**: 1-2

### getlyrics
| Параметр | Описание |
|----------|----------|
| `id` | ID песни |

### setlyrics
| Параметр | Описание |
|----------|----------|
| `id` | ID песни |
| `lyrics` | Текст |

---

## 13. SYNO.AudioStation.Radio

**Path**: `AudioStation/radio.cgi`
**Versions**: 1-2

### list
Список интернет-радиостанций.

### add
| Параметр | Описание |
|----------|----------|
| `title` | Название |
| `url` | URL потока |

### search (v2+)
Поиск радиостанций в онлайн-каталогах.

---

## 14. SYNO.AudioStation.Folder

**Path**: `AudioStation/folder.cgi`
**Versions**: 1-3

### list
| Параметр | Описание |
|----------|----------|
| `id` | ID папки (корень если не указан) |
| `offset`, `limit` | Пагинация |
| `additional` | Доп. метаданные |

---

## 15. SYNO.AudioStation.Genre / Composer

**Genre path**: `AudioStation/genre.cgi` (v1-3)
**Composer path**: `AudioStation/composer.cgi` (v1-2)

### list
Пагинация и сортировка.

---

## 16. SYNO.AudioStation.Download

**Path**: `AudioStation/download.cgi`
**Version**: 1

### download
| Параметр | Описание |
|----------|----------|
| `id` | ID песни |

Ответ: оригинальный аудиофайл.

---

## Источники

- [kwent/syno Wiki](https://github.com/kwent/syno/wiki/Audio-Station-API)
- [zzarbi/synology API definitions](https://github.com/zzarbi/synology)
- [N4S4/synology-api Python client](https://github.com/N4S4/synology-api)
- [geloczi/synologydotnet-audiostation](https://github.com/geloczi/synologydotnet-audiostation)
- [DSM Login Web API Guide (Synology)](https://kb.synology.com/DG/DSM_Login_Web_API_Guide/2)
