<p align="center"><img src="assets/logo.jpg" alt="Afflictio" width="180"></p>

# Afflictio

A mobile OS on the Linux kernel (AArch64) with a Rust userspace. The kernel comes from upstream Linux and builds with the repository's config fragment; every system component is written from scratch and none of them depend on systemd.

[![ci](https://github.com/mel0k1/Afflictio/actions/workflows/ci.yml/badge.svg)](https://github.com/mel0k1/Afflictio/actions/workflows/ci.yml)
[![license](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)

[Русский](README.md)

## Status

Roadmap stages 1-3 are done (tags v0.1.0, v0.2.0, v0.3.0). The skeleton boots in QEMU end to end: liinit mounts the filesystems and starts services, tiosessd brings up two autologin sessions on VT1 and VT2, session text shows up on the framebuffer through virtio-gpu, the Ctrl+Alt+Fn combo switches consoles through the HAL-based keyboard listener, a killed daemon comes back through the supervisor, and SIGTERM ends in a clean poweroff. The whole scenario runs in CI in the `qemu` job. The first device target is chosen (PinePhone) and the pmOS artifact pipeline builds in CI in the `device` job: the kernel, DTBs, modules, U-Boot, the kernel config check and rootfs.ext4. The graphics stack and flashing to hardware are still ahead; the plan lives in docs/roadmap.en.md.

## Components

| Component | Purpose |
| --- | --- |
| `liinit` | PID 1: mounts, service supervisor, clean shutdown |
| `tiosessd` | Session manager: autologin on several VTs, Ctrl+Alt+Fn switching, IPC |
| `tioctl` | CLI: status, devices, session start, poweroff and reboot |
| `tiohal` | Library: evdev with Ctrl+Alt+Fn parsing, power_supply, backlight |
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

The smoke test walks the whole path. The `linux-virt` kernel comes from an Alpine apk package, and the same package supplies the `virtio-gpu`, `evdev` and `virtio-input` modules for the initramfs (fbcon and VT are built into the kernel). Binaries build against musl, the initramfs packs an Alpine minirootfs. QEMU runs with `virtio-gpu-pci` and `virtio-keyboard-pci`, keystrokes arrive through the QMP monitor's send-key command. Inside the machine a probe script checks the status (two sessions, keyboard counter above zero), switches consoles with Ctrl+Alt+F1 and Ctrl+Alt+F2 and compares the content of `/dev/fb0` between switches: the frames must differ, which proves session text really reaches the screen. Then tiosessd is killed, the supervisor brings it back, SIGTERM takes the machine to poweroff, and QEMU exits with code 0.

```sh
./scripts/qemu-smoke.sh
```

Requirements: `qemu-system-aarch64`, `cpio`, `curl`, `python3`, `cargo`. Environment variables: `KERNEL_APK_URL`, `MINIROOTFS_URL` (Alpine 3.22 by default), `QEMU_DATA_DIR` (a QEMU firmware directory, when ROM files live somewhere unusual). Artifacts stay in `out/`: the kernel, the modules, the initramfs, the serial log.

Building the kernel from source with the config fragment is described in [kernel/README.md](kernel/README.md).

## Devices

The first target is the [PinePhone](devices/pinephone/README.en.md). The stage 3 scripts pull postmarketOS pipeline artifacts (the 6.17.5 kernel, DTBs, 431 modules, U-Boot) from pinned apk packages, extract the exact kernel config from the embedded IKCONFIG stream and check it against 16 requirements of our userspace, then pack the boot directory, U-Boot and rootfs.ext4 into an SD card bundle. The pipeline is verified in CI; flashing to hardware belongs to stages 4-5.

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

An example config with two autologin sessions and a service list lives in kernel/initramfs/.

## Documentation

- [Architecture (EN)](docs/architecture.en.md) / [Архитектура (RU)](docs/architecture.ru.md)
- [Roadmap (EN)](docs/roadmap.en.md) / [Дорожная карта (RU)](docs/roadmap.ru.md)
- [Kernel](kernel/README.md)
- [PinePhone (EN)](devices/pinephone/README.en.md) / [PinePhone](devices/pinephone/README.md)

## License

GPL-3.0-or-later, full text in [LICENSE](LICENSE).
