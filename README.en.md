<p align="center"><img src="assets/logo.jpg" alt="Afflictio" width="180"></p>

# Afflictio

A mobile OS on the Linux kernel (AArch64) with a Rust userspace. The kernel comes from upstream Linux and builds with the repository's config fragment; every system component is written from scratch and none of them depend on systemd.

[![ci](https://github.com/mel0k1/Afflictio/actions/workflows/ci.yml/badge.svg)](https://github.com/mel0k1/Afflictio/actions/workflows/ci.yml)
[![license](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)

[Русский](README.md)

## Status

The skeleton already boots end to end in QEMU: liinit mounts the filesystems and starts services, tiosessd brings up an autologin session on a virtual terminal, tioctl talks to the daemon over a unix socket, a killed daemon comes back through the supervisor, and SIGTERM to PID 1 takes the machine through a clean poweroff. The scenario runs both locally and in CI (the `qemu` job). Roadmap stage 1 is done and tagged v0.1.0. The graphics stack and per-device images are still ahead; the plan lives in docs/roadmap.en.md.

## Components

| Component | Purpose |
| --- | --- |
| `liinit` | PID 1: mounts, service supervisor, clean shutdown |
| `tiosessd` | Session manager: autologin, VT switching, IPC over a unix socket |
| `tioctl` | CLI: status, devices, session start, poweroff and reboot |
| `tiohal` | Library: evdev, power_supply, backlight |
| `lioipc` | Shared protocol: JSON messages for tiosessd and tioctl |

The names are fused from pieces of the word Afflictio: `li` + init, `tio` + hal and so on. Configs live in `/etc/afflictio/`, the socket at `/run/afflictio/tiosess.sock`, and the kernel parameter is `afflictio.init=`.

## Building

Any current Rust toolchain works; the crates keep a minimal dependency set (`libc`, `serde`, `toml`, `signal-hook`):

```sh
cargo build --release
```

The binaries land in `target/release/liinit`, `target/release/tiosessd`, `target/release/tioctl`.

For the target architecture the crates build against the musl target with static linking through `rust-lld`, no cross-gcc needed:

```sh
rustup target add aarch64-unknown-linux-musl
cargo build --release --target aarch64-unknown-linux-musl
```

## Running in QEMU

The smoke test walks the whole path: it builds the aarch64 binaries, packs an initramfs from an Alpine minirootfs, downloads the `linux-virt` kernel from the Alpine netboot repository, unwraps the EFI zboot payload and boots everything in `qemu-system-aarch64 -machine virt`. Inside, liinit starts tiosessd with autologin, a probe script talks to the IPC, kills tiosessd and waits for the supervisor to respawn it, then sends SIGTERM to PID 1. The machine powers down, QEMU exits with code 0, and the serial log keeps the `SMOKE_IPC_OK`, `SMOKE_SESSIONS_OK`, `SMOKE_RESPAWN_OK` markers.

```sh
./scripts/qemu-smoke.sh
```

Requirements: `qemu-system-aarch64`, `cpio`, `curl`, `cargo`. Environment variables: `KERNEL_URL`, `MINIROOTFS_URL` (Alpine 3.22 by default), `QEMU_TIMEOUT`, `QEMU_DATA_DIR` (a QEMU firmware directory, when ROM files live somewhere unusual). Artifacts stay in `out/`: the kernel, the initramfs, the serial log.

Building the kernel from source with the config fragment is described in [kernel/README.md](kernel/README.md).

## Running without QEMU

The skeleton runs on any machine. In a terminal:

```sh
cargo run -p liinit
```

The liinit notices it is not PID 1, skips mounting and works as a plain supervisor driven by its TOML config. The session daemon starts the same way; the socket path is overridden with `AFFLICTIO_SOCKET`:

```sh
cargo run -p tiosess
AFFLICTIO_SOCKET=/tmp/tiosess.sock ./target/release/tioctl status
```

Example configs with autologin and a service list live in kernel/initramfs/.

## Documentation

- [Architecture (EN)](docs/architecture.en.md) / [Архитектура (RU)](docs/architecture.ru.md)
- [Roadmap (EN)](docs/roadmap.en.md) / [Дорожная карта (RU)](docs/roadmap.ru.md)
- [Kernel](kernel/README.md)

## License

GPL-3.0-or-later, full text in [LICENSE](LICENSE).
