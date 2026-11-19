# Afflictio Architecture

## Overview

Afflictio pairs the Linux kernel (AArch64) with a set of Rust components in userspace. The kernel builds from upstream with the repository's config fragment, and the userspace is a single workspace of five crates. The session daemon and the control utility talk over a unix socket with line-framed JSON messages.

Boot works like this: the loader hands the kernel `Image` plus an initramfs, the kernel unpacks the archive and runs `/init` from it, which execs into `liinit` as PID 1. The init finds its config through the kernel parameter `afflictio.init=` (default `/etc/afflictio/init.toml`), mounts the pseudo-filesystems (`/proc`, `/sys`, `/dev`, `/run`) and starts the services listed there.

## Components

| Crate | Role |
| --- | --- |
| `liinit` | PID 1: mounts, service supervisor, SIGCHLD/SIGTERM handling |
| `tiohal` | Input and power library: evdev over epoll, sysfs for battery and backlight |
| `tiosess` | The `tiosessd` daemon: VT sessions, IPC server |
| `tioctl` | The `tioctl` utility: status, devices, sessions, power |
| `lioipc` | Request and response types, JSON line codec, socket client |

### liinit

The supervisor keeps the service list from the TOML config and a table of running children. On every SIGCHLD it reaps status with `waitpid(-1, WNOHANG)` and restarts a crashed service when its spec says `respawn = true`. SIGTERM and SIGINT trigger shutdown: init kills all children, waits for them and calls `reboot(2)` with `LINUX_REBOOT_CMD_POWER_OFF` when it runs as the real PID 1.

Running outside PID 1 (for example `cargo run -p liinit` in a terminal) skips mounting and behaves like a plain supervisor, which makes the skeleton testable without a virtual machine.

### tiohal

The HAL deliberately avoids ready-made driver crates. Input is read straight from `/dev/input/eventN`: the device opens with `O_NONBLOCK`, `input_event` structures are parsed byte by byte in the little-endian layout of 64-bit ARM, and epoll watches all descriptors at once. The whole layer is a few hundred lines and compiles unchanged for both x86_64 and AArch64.

Power and backlight come from sysfs: `/sys/class/power_supply/*/capacity` and `status`, `/sys/class/backlight/*/brightness`. Writing to `brightness` controls the display light; the value is clamped to `max_brightness` first.

### tiosess

`tiosessd` owns the session list. A session is a process detached with `setsid()` and attached to `/dev/ttyN` through `TIOCSCTTY`; console switching goes through `VT_ACTIVATE` and `VT_WAITACTIVE` on `/dev/tty0`. The config supports autologin: the given user and command start on VT 1 when the daemon starts. When the supervisor respawns the daemon, a repeated autostart onto the already occupied VT is rejected by the kernel and the log records it: session state does not survive a daemon restart.

The socket lives at `/run/afflictio/tiosess.sock` by default; the path can be overridden by config or the `AFFLICTIO_SOCKET` variable. The protocol is one JSON line per request and one per response. The types live in `lioipc` and are covered by round-trip tests.

### tioctl

`tioctl` works in two modes. When tiosessd answers, the `status`, `session` and `switch` commands go through the socket. When it does not, `status` falls back to reading `/proc/uptime` and the kernel release. The `devices` command needs no daemon: it calls the HAL in-process and lists input devices, batteries and backlights.

`poweroff` and `reboot` call `reboot(2)` and therefore need root. In a full system these commands will be reachable only through a privileged path; for now the restriction matches the syscall itself.

## Smoke test in QEMU

Roadmap stage 1 verifies the whole vertical with one script, `scripts/qemu-smoke.sh`: aarch64-musl binaries statically linked with `rust-lld`, an initramfs built from an Alpine minirootfs with our binaries and configs, the `linux-virt` kernel from the Alpine netboot repository, booted in `qemu-system-aarch64 -machine virt`. Inside the machine a probe script waits for the tiosessd socket, reads the status, kills the daemon and confirms the supervisor brought it back, then sends SIGTERM to PID 1. Success is recorded by the `SMOKE_IPC_OK`, `SMOKE_SESSIONS_OK`, `SMOKE_RESPAWN_OK` markers in the serial log and by a zero QEMU exit code after a clean poweroff. The same script runs in CI in the `qemu` job.

## Deliberately out of scope

The graphics stack, DHCP networking, package installation and per-device images are not part of the skeleton. The work order and the approach to libhybris, libgbinder and postmarketOS patches are described in docs/roadmap.en.md. This is a skeleton rather than a distribution: the goal is to verify every layer on its own before composing them together.
