# PinePhone: the first Afflictio target

PinePhone (PINE64, Allwinner A64) is the first device for roadmap stage 3. Rationale: a mainline kernel with strong postmarketOS support (the `linux-postmarketos-allwinner` package, 431 modules), mainline U-Boot boot, booting from microSD without touching the internal flash, and a cheap, common dev device. The artifact pipeline and the distribution build are verified in CI in the `device` job; flashing to hardware stays for the bring-up session.

## Artifacts from the postmarketOS pipeline

`scripts/fetch-pm-kernel.sh` pulls ready-made pmOS pipeline artifacts (pmbootstrap builds the same apk packages from aports), with pinned versions and an APKINDEX fallback:

| Artifact | Version | Contents |
| --- | --- | --- |
| `linux-postmarketos-allwinner` | 6.17.5_git20251026-r2 | 24 MB Image, PinePhone 1.1/1.2/1.2b DTBs, 431 modules |
| `u-boot-pinephone` | 2023.01-r4 | u-boot-sunxi-with-spl-{528,552,624}.bin (SD/eMMC) |

The kernel config is not published as a separate file, but the pmOS kernel builds with `CONFIG_IKCONFIG`: the script finds the `IKCFG_ST` marker in the Image, unpacks the embedded gzip stream and saves the exact config to `out/device/pinephone/kernel.config` (2322 options). The check therefore runs against the configuration of the kernel that was actually built, without touching the pmOS GitLab that sits behind anti-bot protection.

## Compatibility check

`scripts/check-kernel-config.sh` compares `kernel/device-requirements.txt` against the extracted config: 19 requirements, 13 mandatory and 6 preferred. Against the pmOS v25.12 kernel every mandatory one holds (devtmpfs, proc, sysfs, tmpfs, UNIX, ELF, VT and framebuffer_console, ext4, evdev, the USB-serial set of the modem). The config also shows that `CONFIG_EXT4_FS=y`, `CONFIG_MMC_SUNXI=y`, `CONFIG_MMC_BLOCK=y` and `CONFIG_DRM_SUN4I=y` are built into the kernel, so the rootfs mounts without an initramfs and `init=/sbin/liinit` runs directly. The single warning is the old one:

- `CONFIG_ANDROID_BINDERFS=n` while `CONFIG_ANDROID_BINDER_IPC=y`: Binder is present, but the binderfs mount gets enabled by our config fragment when the pmOS kernel is rebuilt.

The pmOS kernel modules are zstd-compressed (`.ko.zst`) and the kernel is built without `CONFIG_MODULE_DECOMPRESS`: `scripts/build-rootfs.sh` unpacks both module trees (pmOS via `zstd`, virt via `gzip`) and rewrites `modules.dep`/`modules.alias` to the unpacked paths, so busybox modprobe works with ready-made `.ko` files.

## Rootfs

`scripts/build-rootfs.sh` assembles the distribution rootfs (in CI in the `device` job):

- base: alpine minirootfs aarch64 (busybox, musl, apk for extending on the device);
- Afflictio packages: `liinit` as `/sbin/init`, `tiosessd`, `tioctl`, `tiocompd`, `binderctl`, `lioatd`, `modemctl`, `probe.sh`;
- users: root and `mel0k1` (uid 1000, passwords locked, autologin through tiosessd with a real setuid);
- both module trees: `6.12.118-0-virt` for QEMU and `6.17.5` for PinePhone;
- `/etc/afflictio/init.toml` with `[[modules]]`, `[[mounts]]` (binderfs) and the lioatd/tiosessd/tiocompd/probe services, `/etc/afflictio/lioat.toml` with the modem device list;
- `/boot` with the Image and three DTBs, `/extlinux/extlinux.conf` with `root=UUID`;
- `/etc/afflictio/manifest.json` with the version and package list;
- `out/rootfs.ext4`: ext4 with a fixed UUID, `/home/mel0k1` gets uid 1000 through `debugfs sif`.

The same rootfs boots in QEMU in the `rootfs` job (`scripts/qemu-rootfs-smoke.sh`): a mini-initramfs (`scripts/mkrootfs-initramfs.sh`) brings up `virtio_blk` and `ext4`, runs `switch_root` onto rootfs.ext4, and from there PID 1 is the real `liinit` from the rootfs. The scenario passes with the SMOKE_READY, SMOKE_SWITCH_VT1_OK, SMOKE_SWITCH_VT2_OK, SMOKE_FRAMES_DIFFER, SMOKE_UID_OK (proof that sessions run as uid 1000), SMOKE_KMS_FLIPS_OK (the compositor on virtio-gpu), SMOKE_RESPAWN_OK, `SMOKE_MODEM_AT_OK` (the modem stub on the second PL011) markers and a clean poweroff.

## The modem

The PinePhone carries a Quectel EG25-G on USB; the pmOS mainline kernel builds its drivers as modules (`CONFIG_USB_OPTION`, `CONFIG_USB_NET_QMI_WWAN` are in the mandatory requirement list as `=y|m`), and the modem AT port shows up as `/dev/ttyUSB2`. The `lioatd` daemon reads `/etc/afflictio/lioat.toml` and opens the first available port from the `devices` list — on hardware that is ttyUSB2, in QEMU the second PL011 `/dev/ttyAMA1`. Probing every `probe_interval` seconds refreshes the snapshot: the model, the SIM state (`READY`), the signal level in CSQ and dBm, the network registration; `modemctl status` prints it as JSON and `modemctl at "AT+..."` sends an arbitrary command through the daemon unix socket. In the QEMU smoke a real EG25-G is replaced by `scripts/modem-emu.py` — the stub answers the same command set, and lioatd records `SMOKE_MODEM_AT_OK ... pin=READY rssi=21`. Data over QMI and the check for a disappearing USB device stay for bring-up.

## SD-card image

`scripts/pack-device-bundle.sh` builds the bundle and three flashable images:

```
out/device/pinephone/
  bundle/
    boot/                     Image, DTB 1.1/1.2/1.2b, extlinux/extlinux.conf
    uboot/pine64-pinephone/   u-boot-sunxi-with-spl-{528,552,624}.bin
    rootfs/                   the rootfs tree (the same one inside rootfs.ext4)
    rootfs.ext4
  afflictio-pinephone-528.img
  afflictio-pinephone-552.img
  afflictio-pinephone-624.img
```

Each image: an MBR with one ext4 partition (type 0x83, LBA 2048), `u-boot-sunxi-with-spl` at offset 8192 bytes (sector 16, the sunxi standard), and the `rootfs.ext4` content from the partition start. Inside the partition sit the rootfs, `/boot` and `/extlinux/extlinux.conf`: u-boot reads ext4 directly, and `fdt` is not set in extlinux because each u-boot variant passes its own DTB matching the board revision (1.1, 1.2, 1.2b).

Image verification (`scripts/assemble-img.py verify` plus debugfs): the MBR signature, the `eGON.BT0` magic at offset 8192, the ext4 superblock at the partition start, binaries and passwd inside the partition, `/boot/Image`, extlinux with `root=UUID`, the manifest. mke2fs, debugfs and python work without root, so the pipeline runs in CI.

## Flashing the SD card (recipe for the hardware session)

Not verified on the device, done manually at bring-up:

```sh
# figure out the SD device (last dmesg lines after insertion), then:
sudo dd if=out/device/pinephone/afflictio-pinephone-624.img of=/dev/sdX bs=4M conv=fsync status=progress
sync
```

The u-boot variant inside the image determines the memory timings; the variant-to-board-revision mapping is confirmed at bring-up (in the pmOS u-boot-pinephone package the 624 variant is used for the newer revisions). On the boot console (ttyS0, 115200 8N1, the UART connector on the bottom edge) the kernel and liinit output is visible; on the PinePhone display the tiocompd compositor frame appears on top of the Allwinner KMS, and shell sessions for user mel0k1 open on tty1 and tty2.

Quick reboot into u-boot without the SD card: u-boot on the board can hold `fastboot 0`, so at bring-up the eMMC can be flashed through `fastboot flash` (the gadget is on USB-C; `fastboot devices` on the host should show the PinePhone). SD boot does not require fastboot.

Expected bring-up debug points: the backlight module (pwm-backlight is in the pmOS DTB), buttons and keyboard through the USB-C dock (evdev), matching `root=UUID` to the actual partition (the UUID is fixed in build-rootfs.sh and extlinux.conf), and the binderfs mount behavior (expected to be skipped with a log line until the pmOS kernel is rebuilt).

## What is next

Stage 5 is assembled: a rootfs with users and a manifest, flashable images with verification, and the rootfs boot in QEMU. What remains of the stage: flashing to hardware and tuning the `[[modules]]` list to the real PinePhone device order at bring-up.
