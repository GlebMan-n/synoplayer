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
├── CLAUDE.md               # Конвенции для AI-агентов
├── AGENTS.md               # Руководство по разработке
├── README.md
├── docs/
│   ├── API_REFERENCE.md    # Справочник Synology API
│   ├── SPECIFICATION.md    # Спецификация разработки
│   ├── AS_Guide.pdf        # Synology Lyrics Module Guide
│   └── DSM_Developer_Guide_7_enu.pdf
└── src/                    # (будет создан)
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
