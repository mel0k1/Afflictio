# Afflictio Architecture

## Overview

Afflictio pairs the Linux kernel (AArch64) with a set of Rust components in userspace. The kernel builds from upstream with the repository's config fragment, and the userspace is a single workspace of nine crates. The daemons and the control utilities talk over unix sockets with line-framed JSON messages.

Boot works like this: the loader hands the kernel `Image` plus an initramfs, the kernel unpacks the archive and runs `/init` from it, which execs into `liinit` as PID 1. The init finds its config through the kernel parameter `afflictio.init=` (default `/etc/afflictio/init.toml`), mounts the pseudo-filesystems (`/proc`, `/sys`, `/dev`, `/run`), loads device modules (`virtio-gpu`, `evdev`, `virtio-input` in the QEMU environment) and starts the services listed there.

## Components

| Crate | Role |
| --- | --- |
| `liinit` | PID 1: mounts (including configurable [[mounts]]), service supervisor, SIGCHLD/SIGTERM handling |
| `tiohal` | Input and power library: evdev over epoll, a Ctrl+Alt+Fn tracker, sysfs for battery and backlight |
| `tiosess` | The `tiosessd` daemon: VT sessions, keyboard-driven console switching, IPC server |
| `tioctl` | The `tioctl` utility: status, devices, sessions, power |
| `lioipc` | Request and response types, JSON line codec, socket client |
| `tiocomp` | The `tiocompd` daemon: compositor on top of KMS/DRM, dumb buffers, page flips |
| `liobinder` | Binder transport: parcels, `BINDER_WRITE_READ`, servicemanager client, the `binderctl` utility |
| `lioat` | The `lioatd` daemon and `modemctl`: AT channel on termios, the 27.007 probe, device-list fallback |
| `liota` | A/B updates: slot status, switching through extlinux, payload install |

### liinit

The supervisor keeps the service list from the TOML config and a table of running children. On every SIGCHLD it reaps status with `waitpid(-1, WNOHANG)` and restarts a crashed service when its spec says `respawn = true`. SIGTERM and SIGINT trigger shutdown: init kills all children, waits for them and calls `reboot(2)` with `LINUX_REBOOT_CMD_POWER_OFF` when it runs as the real PID 1.

Running outside PID 1 (for example `cargo run -p liinit` in a terminal) skips mounting and behaves like a plain supervisor, which makes the skeleton testable without a virtual machine.

### tiohal

The HAL deliberately avoids ready-made driver crates. Input is read straight from `/dev/input/eventN`: the device opens with `O_NONBLOCK`, `input_event` structures are parsed byte by byte in the little-endian layout of 64-bit ARM, and epoll watches all descriptors at once. The whole layer is a few hundred lines and compiles unchanged for both x86_64 and AArch64.

On top of the raw events sits `ComboTracker`: it watches Ctrl and Alt across `EV_KEY` events (autorepeat is ignored, releasing resets the modifiers) and reports a virtual console number when an F1..F8 key goes down. The tracker is pure logic and carries unit tests for key order, autorepeat and range edges.

Power and backlight come from sysfs: `/sys/class/power_supply/*/capacity` and `status`, `/sys/class/backlight/*/brightness`. Writing to `brightness` controls the display light; the value is clamped to `max_brightness` first.

### tiosess

`tiosessd` owns the session list. A session is a process detached with `setsid()` and attached to `/dev/ttyN` through `TIOCSCTTY`; the tty opens read-write because the same fd becomes the session's stdin, stdout and stderr. Console switching goes through `VT_ACTIVATE` and `VT_WAITACTIVE` on `/dev/tty0`, a request for the already active console is ignored, and the range is capped at VT1..VT12.

The config supports several autologin entries through `[[autologin]]`: the given user and command start on their console when the daemon boots, with the console number set by the `vt` field or assigned in order. When the supervisor respawns the daemon, autostart runs again: an activation onto a console held by another process is rejected by the kernel and the log records it, and the previous session state does not survive the restart.

Keyboard switching runs in its own thread: every second it rescans `/dev/input`, adds new devices to a shared epoll and feeds events through `ComboTracker`; a triggered combo becomes a `Switch(vt)` message sent over an mpsc channel to the main loop, so the same `SessionManager` that serves IPC performs the switch. The counter of found keyboards publishes through `Arc<AtomicU32>` and shows up in status responses as `keyboard_devices`.

The socket lives at `/run/afflictio/tiosess.sock` by default; the path can be overridden by config or the `AFFLICTIO_SOCKET` variable. The protocol is one JSON line per request and one per response. The types live in `lioipc` and are covered by round-trip tests.

### tioctl

`tioctl` works in two modes. When tiosessd answers, the `status`, `session` and `switch` commands go through the socket. When it does not, `status` falls back to reading `/proc/uptime` and the kernel release. The `devices` command needs no daemon: it calls the HAL in-process and lists input devices, batteries and backlights.

`poweroff` and `reboot` call `reboot(2)` and therefore need root. In a full system these commands will be reachable only through a privileged path; for now the restriction matches the syscall itself.

## Smoke test in QEMU

The roadmap verifies the whole vertical with one script, `scripts/qemu-smoke.sh`. Preparation: `scripts/prepare-kernel.sh` pulls the kernel and a module set (DRM virtio-gpu, evdev, virtio-input; fbcon and VT are built in) from the `linux-virt` apk package, `scripts/mkinitramfs.sh` assembles an initramfs from an Alpine minirootfs with our binaries, configs and modules. Execution: QEMU with `virtio-gpu-pci` and `virtio-keyboard-pci`, keystrokes go through the QMP send-key command.

Inside the machine a probe script waits until the status shows two sessions and a nonzero keyboard counter, then verifies switching: the host sends Ctrl+Alt+F1 and Ctrl+Alt+F2, the probe waits for the active console change in status and takes a copy of `/dev/fb0` on each console. The copies compare byte by byte: a difference above the threshold means session text was actually drawn on each console's framebuffer. The probe then kills tiosessd and confirms the supervisor brought it back, before sending SIGTERM to PID 1. Success is recorded by the `SMOKE_READY`, `SMOKE_SWITCH_VT1_OK`, `SMOKE_SWITCH_VT2_OK`, `SMOKE_FRAMES_DIFFER`, `SMOKE_KMS_FLIPS_OK`, `SMOKE_RESPAWN_OK` markers (plus the informational `SMOKE_BINDERFS_ABSENT`) in the serial log and by a zero QEMU exit code after a clean poweroff. The same script runs in CI in the `qemu` job.

## Device artifacts

Stage 3 moves the "artifacts from apk packages" scheme onto real hardware, still without running pmbootstrap. `scripts/fetch-pm-kernel.sh` downloads pinned apk packages from the postmarketOS mirror (`linux-postmarketos-allwinner` and `u-boot-pinephone`) and unpacks them into `out/device/pinephone/`: the kernel Image, PinePhone DTBs for three board revisions, 431 kernel modules and three `u-boot-sunxi-with-spl` variants. pmOS does not publish the kernel config as a separate file, but the kernel builds with `CONFIG_IKCONFIG`: the script finds the `IKCFG_ST` marker in the Image and unpacks the embedded config, so the checks run against the configuration of the kernel that was actually built.

`scripts/check-kernel-config.sh` compares `kernel/device-requirements.txt` against the extracted config: 13 mandatory userspace requirements (devtmpfs, proc, sysfs, tmpfs, UNIX, ELF, VT, framebuffer_console, ext4, evdev plus the USB-serial set of the modem: USB_SERIAL, USB_SERIAL_OPTION, USB_NET_QMI_WWAN) and 6 preferred ones. Against the pmOS v25.12 kernel every mandatory one holds, and the single warning `CONFIG_ANDROID_BINDERFS=n` moved into the stage 4 plan: the fragment in kernel/ supplements the pmOS defconfig rather than replacing it.

`scripts/pack-device-bundle.sh` assembles the SD card bundle: a boot directory (Image, DTB, extlinux.conf with `init=/sbin/liinit`), u-boot-sunxi-with-spl and `rootfs.ext4`, where busybox, our binaries, `/etc/afflictio` and the modules are packed with `mke2fs -d` into ext4 without root. The assembled image is verified with `debugfs` from the inside: binaries and the session config read back. All three steps run in CI in the `device` job; flashing to hardware stays with the bring-up session (the recipe lives in devices/pinephone/README.md).

## The compositor

Graphics on top of KMS belong to the `tiocompd` daemon from the `tiocomp` crate. The KMS/DRM layer is written from scratch on libc ioctls: opening `/dev/dri/cardN`, `GETRESOURCES`/`GETCONNECTOR` with the two-phase protocol (the first call with `count_modes=1` and a single `modeinfo` slot returns the counters, the second fills the arrays), `GETENCODER`/`GETCRTC`, `CREATE_DUMB` + `MAP_DUMB` + `mmap` for two buffers, `ADDFB2` (XRGB8888) and `SETCRTC`/`PAGE_FLIP`. The ioctl numbers match the current kernel uapi: `GETCONNECTOR` is `0xA7`, `GETENCODER` `0xA6`, `RMFB` `0xAF`, and the structs carry no pad fields (`drm_mode_card_res` is 64 bytes). When a driver does not support `PAGE_FLIP`, frames switch through a full `SETCRTC` and the method is recorded in the status.

Frame composition is pure arithmetic and covered by unit tests: the background, two session windows with a border and a moving animation bar, the title and the frame counter are drawn with a scaled 5x7 font. The daemon writes `/run/afflictio/tocomp.json` every 10 frames (`connector`, `mode`, `method`, `flips`, `frames`); the QEMU smoke test waits for `flips > 20` and records the `SMOKE_KMS_FLIPS_OK` marker. On SIGTERM `tiocompd` returns the CRTC to the original fbcon framebuffer and removes its own fbs and dumb buffers. Without a KMS device the daemon exits quietly with code zero, so the scenario survives on a graphics-less machine.

## Binder

The Binder transport layer lives in the `liobinder` crate without dependencies beyond libc. Parcel primitives: 4-byte alignment, `string16` in UTF-16LE, `flat_binder_object` (types `0x73627242/0x73627248`), an object offset table. Transport: `BINDER_VERSION`, a 1 MB mmap of the kernel buffer (required before the first transaction, or the kernel has nowhere to put the reply), `BINDER_WRITE_READ` with parsing of BR commands (both layouts of `BR_TRANSACTION`/`BR_REPLY`, plain and sg, for kernels before and after 5.0), and freeing incoming buffers through `BC_FREE_BUFFER`. The `servicemanager` module implements the `GET_SERVICE` request in the legacy protocol (the `android.os.IServiceManager` interface token plus the service name): it is a draft, verifiable only against a real servicemanager on the device, since vendor stacks carry their own layout.

binderfs mounting is configured in init.toml through `[[mounts]]` (the new liinit section: source, target, fstype; a failed mount is logged and does not abort boot). The `binderctl probe` utility creates a node through `binder-control` or opens the legacy `/dev/binder` and prints the protocol version. The `linux-virt` kernel in QEMU has no Binder, so the smoke test prints the informational `SMOKE_BINDERFS_ABSENT` marker and does not treat it as a failure; the pmOS PinePhone kernel carries `BINDER_IPC=y`, and `BINDERFS` needs our config fragment. Numeric command and ioctl codes are pinned by tests against reference values (for example `BINDER_WRITE_READ = 0xC0306201`), and parcels and offsets are verified by roundtrip tests.

## The modem

The AT layer lives in the `lioat` crate without dependencies beyond libc and serde. The `AtChannel` opens a serial device with `O_NONBLOCK|O_NOCTTY`, switches it to raw mode through termios (B115200, CLOCAL|CREAD) and holds the conversation on poll with a deadline: a command goes out as a frame with `\r`, the reply is collected line by line until the final `OK`/`ERROR`/`+CME ERROR`, echo is stripped, and incomplete lines stay in the channel buffer across calls — replies of consecutive commands and URCs survive when they coalesce into a single read. On top of the channel runs the 27.007 probe: `ATE0`, `AT+CGMI`, `AT+CGMM`, `AT+CPIN?`, `AT+CSQ`, `AT+CREG?`; the results (manufacturer, model, READY/SIM PIN/SIM PUK, the signal level translated to dBm, the registration) go into a `ModemState` snapshot.

The `lioatd` daemon reads `/etc/afflictio/lioat.toml`, opens the first available port from the `devices` list (on the PinePhone `/dev/ttyUSB2` behind the option driver, in QEMU the second PL011 `/dev/ttyAMA1`), re-probes the modem every `probe_interval` seconds and owns the unix socket `/run/afflictio/modem.sock`: `modemctl status` returns a JSON snapshot, `modemctl at CMD` forwards an arbitrary command into the channel. The protocol is pinned by round-trip tests; when the device disappears the daemon closes the channel and goes back to waiting, so a vanishing modem neither breaks the boot nor kills the service. The first successful probe prints the `SMOKE_MODEM_AT_OK path=... manufacturer=... model=... pin=... rssi=...` marker; in QEMU the `scripts/qemu-rootfs-smoke.sh` script catches it, and a real modem is replaced by the `scripts/modem-emu.py` stub.

## A/B updates

The update model is settled as A/B slots, and the whole layer lives in the `liota` crate without a daemon. The image layout: p1 is boot (Image, DTBs, extlinux.conf with labels a and b), p2 is slot a, p3 is slot b; the `slots.toml` config keeps the base disk, the boot partition number and the name/partition/UUID map, and the partition path follows the kernel naming rule (`/dev/vda` → `vda2`, `/dev/mmcblk0` → `mmcblk0p1`), so one config describes both QEMU and hardware. `liota status` takes a JSON snapshot (the active slot from the `root=UUID=` parameter of /proc/cmdline, the inactive one, boot-ok), `liota switch` mounts the boot partition through a libc mount and rewrites the `default` in extlinux.conf by writing a temporary file and renaming it over (atomically), `liota install PAYLOAD` writes the payload into the inactive slot's partition with fsync.

A slot confirms its own boot with a marker: liinit writes `/var/lib/afflictio/boot-ok` after the services start, and status shows its presence. Slot resolution in the boot path goes through extlinux honestly: the mini-initramfs of the rootfs scenario mounts the boot partition, takes the `default` label, matches its `root=UUID=` against slots.toml and mounts the chosen partition — the same decision u-boot makes on hardware. The smoke in the `rootfs` job runs the full cycle across two boots: status and switch on slot a, then a second QEMU run with the same disk lands on slot b and checks the boot-ok marker. What remains: payload delivery over the network with signing and bootcount-based rollback in u-boot env.

## Rootfs and the device image

The distribution stage is three scripts. `scripts/build-rootfs.sh` assembles the rootfs: alpine minirootfs as the base, the Afflictio packages (liinit as /sbin/init, tiosessd, tioctl, tiocompd, binderctl, lioatd, modemctl, liota, probe.sh), the root and mel0k1 users (uid 1000, locked passwords, autologin through tiosessd with a real setuid via getpwnam), two module trees (6.12.118-0-virt for QEMU and 6.17.5 for PinePhone), an init.toml with `[[modules]]`, `[[mounts]]` and the services, a lioat.toml with the modem port list, a slots.toml with the A/B slot map, /boot as the boot partition mountpoint, the /etc/afflictio/manifest.json manifest and an ext4 with the fixed slot a UUID. The /home/mel0k1 ownership is set through debugfs sif after mke2fs -d, because mke2fs cannot change owners.

Both module trees are unpacked at build time (gzip and zstd): the kernels are built without CONFIG_MODULE_DECOMPRESS, and busybox modprobe in this configuration hands the kernel raw bytes, which shows up as Invalid ELF header magic. After unpacking, modules.dep and modules.alias are rewritten and the .bin tables are dropped. liinit gained a `[[modules]]` section: after the mounts it runs modprobe by an absolute path (/sbin/modprobe when present), and load failures are logged without aborting the boot; with no PATH for PID 1 this is the only reliable way.

The rootfs boot in QEMU is verified in the `rootfs` job (`scripts/qemu-rootfs-smoke.sh` plus `scripts/mkrootfs-initramfs.sh`): the image is assembled from a boot partition and two slots, the mini-initramfs mounts proc/sys/dev, loads virtio_blk and ext4, resolves the slot from the extlinux default and slots.toml, runs switch_root onto the chosen partition, and from there PID 1 is the real liinit from the rootfs. The scenario passes with the SMOKE_READY, SMOKE_SWITCH_VT1_OK, SMOKE_SWITCH_VT2_OK, SMOKE_FRAMES_DIFFER, SMOKE_UID_OK, SMOKE_KMS_FLIPS_OK, SMOKE_RESPAWN_OK, SMOKE_MODEM_AT_OK, SMOKE_AB_STATUS_OK, SMOKE_AB_SWITCH_OK markers and a clean poweroff, and then a second QEMU run with the same disk verifies the slot b boot (SMOKE_AB_BOOT_B_OK). The session uid check goes through the /tmp/afflictio-uid.vtN files written by the session shell itself with id -u, and only applies when the files appear, so the initramfs scenario of the `qemu` job stays unchanged.

The SD-card image (`scripts/assemble-img.py` plus `scripts/pack-device-bundle.sh`): an MBR with three ext4 partitions (a 96 MB boot, two 512 MB slots), u-boot-sunxi-with-spl at offset 8192 (sector 16, the sunxi standard, with the eGON.BT0 magic at +4 inside the header), extlinux.conf with labels a and b in the boot partition, and the rootfs.ext4 content in both slots. Image verification: the MBR signature, three partitions of type 0x83 with rising starts, the eGON.BT0 magic, the ext4 superblock of each partition, then debugfs reads Image and both root=UUID lines from the boot partition and the binaries, passwd, manifest and slots.toml from the slot. All steps build in the CI `device` job without root. The SD flashing recipe through dd and the u-boot fastboot path for eMMC are described in devices/pinephone/README.en.md.

## Deliberately out of scope

DHCP networking, package installation and flashing to hardware are not part of the skeleton: the images are verified structurally (MBR, magics, ext4 content), and the boot on a real board waits for bring-up. The compositor draws its own placeholder windows: wiring session output and application buffers, libhybris for devices with closed drivers, and the exact servicemanager layout are described in docs/roadmap.en.md. Every layer is built and verified on its own before composing them together on hardware.
