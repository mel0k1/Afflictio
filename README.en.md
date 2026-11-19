<p align="center"><img src="assets/logo.jpg" alt="Afflictio" width="180"></p>

# Afflictio

A mobile OS on the Linux kernel (AArch64) with a Rust userspace. The kernel comes from upstream Linux and builds with the repository's config fragment; every system component is written from scratch and none of them depend on systemd.

[![ci](https://github.com/mel0k1/Afflictio/actions/workflows/ci.yml/badge.svg)](https://github.com/mel0k1/Afflictio/actions/workflows/ci.yml)
[![license](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)

[Русский](README.md)

## Status

The project is a skeleton. Working today: init as PID 1 with pseudo-filesystem mounting and a service supervisor, input devices read straight from `/dev/input` over epoll, battery and backlight through sysfs, a session daemon attached to VTs with an IPC server, and the `afflictioctl` client. The graphics stack and per-device images are not built yet; the plan lives in docs/roadmap.en.md.

## Components

| Component | Purpose |
| --- | --- |
| `afflictio-init` | PID 1: mounts, service supervisor, clean shutdown |
| `afflictio-sessiond` | Session manager: autologin, VT switching, IPC over a unix socket |
| `afflictioctl` | CLI: status, devices, session start, poweroff and reboot |
| `afflictio-hal` | Library: evdev, power_supply, backlight |
| `afflictio-ipc` | Shared protocol: JSON messages for sessiond and the CLI |

## Building

Any current Rust toolchain works; the crates keep a minimal dependency set (`libc`, `serde`, `toml`, `signal-hook`):

```sh
cargo build --release
```

The binaries land in `target/release/afflictio-init`, `target/release/afflictio-sessiond`, `target/release/afflictioctl`.

To check against the target architecture:

```sh
rustup target add aarch64-unknown-linux-gnu
cargo check --workspace --target aarch64-unknown-linux-gnu
```

Kernel building is described in [kernel/README.md](kernel/README.md): the script downloads a Linux archive from kernel.org, applies the `kernel/configs/afflictio.fragment` fragment and builds `Image` with the `aarch64-linux-gnu-` toolchain.

## Running without a phone

The skeleton runs on any machine. In a terminal:

```sh
cargo run -p afflictio-init
```

The init notices it is not PID 1, skips mounting and works as a plain supervisor driven by its TOML config. The session daemon starts the same way; the socket path is overridden with `AFFLICTIO_SOCKET`:

```sh
cargo run -p afflictio-session
AFFLICTIO_SOCKET=/tmp/sessiond.sock ./target/release/afflictioctl status
```

Example configs with autologin and a service list will arrive with the initramfs image (stage 1 of the roadmap).

## Documentation

- [Architecture (EN)](docs/architecture.en.md) / [Архитектура (RU)](docs/architecture.ru.md)
- [Roadmap (EN)](docs/roadmap.en.md) / [Дорожная карта (RU)](docs/roadmap.ru.md)
- [Kernel](kernel/README.md)

## License

GPL-3.0-or-later, full text in [LICENSE](LICENSE).
