# PinePhone: the first Afflictio target

PinePhone (PINE64, Allwinner A64) is the first device for roadmap stage 3. Rationale: a mainline kernel with strong postmarketOS support (the `linux-postmarketos-allwinner` package, 431 modules), mainline U-Boot boot, booting from microSD without touching the internal flash, and a cheap, common dev device. Until stage 5 (the distribution) no image is flashed to hardware, but the whole artifact pipeline is verified in CI in the `device` job.

## Artifacts from the postmarketOS pipeline

`scripts/fetch-pm-kernel.sh` pulls ready-made pmOS pipeline artifacts (pmbootstrap builds the same apk packages from aports), with pinned versions and an APKINDEX fallback:

| Artifact | Version | Contents |
| --- | --- | --- |
| `linux-postmarketos-allwinner` | 6.17.5_git20251026-r2 | 24 MB Image, PinePhone 1.1/1.2/1.2b DTBs, 431 modules |
| `u-boot-pinephone` | 2023.01-r4 | u-boot-sunxi-with-spl-{528,552,624}.bin (SD/eMMC) |

The kernel config is not published as a separate file, but the pmOS kernel builds with `CONFIG_IKCONFIG`: the script finds the `IKCFG_ST` marker in the Image, unpacks the embedded gzip stream and saves the exact config to `out/device/pinephone/kernel.config` (2322 options). The check therefore runs against the configuration of the kernel that was actually built, without touching the pmOS GitLab that sits behind anti-bot protection.

## Compatibility check

`scripts/check-kernel-config.sh` compares `kernel/device-requirements.txt` against the extracted config: 16 requirements, 10 mandatory and 6 preferred. Against the pmOS v25.12 kernel every mandatory one holds (devtmpfs, proc, sysfs, tmpfs, UNIX, ELF, **VT and framebuffer_console**, ext4, evdev), with one warning:

- `CONFIG_ANDROID_BINDERFS=n` while `CONFIG_ANDROID_BINDER_IPC=y`: Binder is present, but the binderfs mount needs enabling in our config fragment at stage 4 (libgbinder). This is exactly the case named in the roadmap: the fragment in kernel/ becomes an addition to the pmOS defconfig, not a replacement.

pmOS kernel modules are zstd-compressed (`.ko.zst`): busybox modprobe cannot unpack them, so the hardware bring-up stage needs kmod or a recompressed module set. Recorded on the bring-up list.

## The bundle

`scripts/pack-device-bundle.sh` assembles `out/device/pinephone/bundle/`:

```
bundle/
  boot/
    Image                     pmOS kernel
    sun50i-a64-pinephone-1.2.dtb
    extlinux/extlinux.conf    cmdline: root=/dev/mmcblk0p2 init=/sbin/liinit console=tty1+ttyS0
  uboot/pine64-pinephone/     three u-boot-sunxi-with-spl variants
  rootfs/                     directory prototype: busybox, liinit, tiosessd, tioctl, /etc/afflictio, /lib/modules/6.17.5
  rootfs.ext4                 the same directory packed with mke2fs -d (512 MB ext4, label afflictio)
```

Integrity is verified inside the script with `debugfs`: binaries are read back from the image and session.toml with `[[autologin]]` is present. mke2fs and debugfs run without root, so the pipeline executes in CI.

## Booting (recipe for the hardware session)

Not verified on the device, executed manually during bring-up:

```sh
sfdisk /dev/sdX <<'EOF'
label: dos
start=1M, size=256M, type=83, bootable
start=257M, type=83
EOF
mkfs.ext4 -L boot /dev/sdX1
mkfs.ext4 -L afflictio /dev/sdX2
dd if=uboot/pine64-pinephone/u-boot-sunxi-with-spl-528.bin of=/dev/sdX bs=1024 seek=8 conv=notrunc
mount /dev/sdX1 /mnt/boot && cp -r bundle/boot/* /mnt/boot/
dd if=bundle/rootfs.ext4 of=/dev/sdX2 bs=4M
```

U-Boot boots `extlinux/extlinux.conf` from the first partition, the kernel mounts the rootfs with `/sbin/liinit` as PID 1, liinit starts tiosessd from the device `etc/init.toml`, and the autologin session shows up on the fbcon console through the PinePhone display (the Allwinner KMS path). Expected bring-up debugging points: kmod for `.ko.zst`, module load order (backlight, buttons, keyboard over the USB-C dock) and whether `/dev/mmcblk0p2` matches the SD card (check the mmc0 alias in the DTS).

## What comes next

Stage 4 builds on this bundle: enabling binderfs in the fragment, a compositor on top of the Allwinner KMS display, libgbinder for the modem (eg25-manager in pmOS takes exactly that path). Stage 5 packs the bundle into a full image flashed through pmbootstrap.
