<p align="center"><img src="assets/logo.jpg" alt="Afflictio" width="180"></p>

# Afflictio

Мобильная ОС на ядре Linux (AArch64) с пространством пользователя на Rust. Ядро берется из upstream Linux и собирается с фрагментом конфигурации репозитория, все системные компоненты написаны с нуля и не зависят от systemd.

[![ci](https://github.com/mel0k1/Afflictio/actions/workflows/ci.yml/badge.svg)](https://github.com/mel0k1/Afflictio/actions/workflows/ci.yml)
[![license](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)

[English](README.en.md)

## Состояние

Проект на стадии каркаса. Работают: init как PID 1 с монтированием псевдо-ФС и супервизором служб, чтение устройств ввода напрямую из `/dev/input` через epoll, батарея и подсветка через sysfs, демон сессий с привязкой к VT и IPC-сервером, клиент `afflictioctl`. Графический стек и образы для конкретных устройств не собраны, план работ лежит в docs/roadmap.ru.md.

## Компоненты

| Компонент | Назначение |
| --- | --- |
| `afflictio-init` | PID 1: монтирование, супервизор служб, корректное выключение |
| `afflictio-sessiond` | Менеджер сессий: autologin, VT-переключение, IPC на unix-сокете |
| `afflictioctl` | CLI: статус, устройства, запуск сессий, poweroff и reboot |
| `afflictio-hal` | Библиотека: evdev, power_supply, backlight |
| `afflictio-ipc` | Общий протокол: JSON-сообщения для sessiond и CLI |

## Сборка

Нужен любой актуальный Rust (крейты держат минимальный набор зависимостей: `libc`, `serde`, `toml`, `signal-hook`):

```sh
cargo build --release
```

Бинарные артефакты: `target/release/afflictio-init`, `target/release/afflictio-sessiond`, `target/release/afflictioctl`.

Для проверки под целевой архитектурой:

```sh
rustup target add aarch64-unknown-linux-gnu
cargo check --workspace --target aarch64-unknown-linux-gnu
```

Сборка ядра описана в [kernel/README.md](kernel/README.md): скрипт скачивает архив Linux с kernel.org, применяет фрагмент `kernel/configs/afflictio.fragment` и собирает `Image` с кросс-тулчейном `aarch64-linux-gnu-`.

## Запуск без телефона

Каркас не требует настоящей машины. В терминале:

```sh
cargo run -p afflictio-init
```

Init заметит, что он не PID 1, пропустит монтирование и будет работать как обычный супервизор служб из своего TOML-конфига. Демон сессий запускается так же, путь сокета переопределяется переменной `AFFLICTIO_SOCKET`:

```sh
cargo run -p afflictio-session
AFFLICTIO_SOCKET=/tmp/sessiond.sock ./target/release/afflictioctl status
```

Пример конфигов с autologin и списком служб появится вместе с initramfs-образом (этап 1 дорожной карты).

## Документация

- [Архитектура (RU)](docs/architecture.ru.md) / [Architecture (EN)](docs/architecture.en.md)
- [Дорожная карта (RU)](docs/roadmap.ru.md) / [Roadmap (EN)](docs/roadmap.en.md)
- [Ядро](kernel/README.md)

## Лицензия

GPL-3.0-or-later, полный текст в [LICENSE](LICENSE).
