# Afflictio Roadmap

## Where the project is

Stages 1-3 are done (tags v0.1.0, v0.2.0, v0.3.0). The skeleton boots in QEMU on ARM64 with one script, `scripts/qemu-smoke.sh`, and the same scenario runs in CI: liinit mounts the filesystems and starts services, tiosessd brings up two autologin sessions on VT1 and VT2, the keyboard switches consoles through the HAL with Ctrl+Alt+Fn, session text really lands on the framebuffer through virtio-gpu, the supervisor restarts a killed daemon, and SIGTERM ends in a clean poweroff. The first device target is chosen (PinePhone) and the pmOS artifact pipeline is verified in CI in the `device` job. There is still a long way to an installable phone distribution, and it is more honest to name the steps that separate the skeleton from a working system.

## Stage 1. Skeleton check in QEMU (done, v0.1.0)

Booting a kernel with initramfs in `qemu-system-aarch64 -machine virt` and confirming that init mounts the filesystems, starts services, reacts to signals and shuts the machine down. The test uses the `linux-virt` kernel from the Alpine repository: it is built for emulation and carries everything the virt machine needs. The kernel-from-source build script with the config fragment stays in scripts/build-kernel.sh for real devices. The check joined CI as the `qemu` job.

## Stage 2. Graphics and sessions (done, v0.2.0)

The terminal-only scenario reached an automated check. Text output: fbcon is built into the kernel, the `virtio-gpu` driver loads as a module from the kernel apk package; sessions print text on their VTs and the content of `/dev/fb0` compares byte by byte between the two consoles. Keyboard attachment: `virtio-keyboard-pci` in QEMU, the HAL reads evdev events, `ComboTracker` recognizes Ctrl+Alt+Fn, and a daemon thread rescans devices and sends switches into the main loop. VT switching by keyboard is verified with the QMP send-key command: the `SMOKE_SWITCH_VT1_OK` and `SMOKE_SWITCH_VT2_OK` markers confirm that both sessions switch by keyboard with no commands typed. Along the way a real bug got fixed: the session tty opened read-only, so all session output was silently lost.

## Stage 3. Devices through postmarketOS (done, v0.3.0)

The first target is chosen: PinePhone (Allwinner A64), the pmOS mainline kernel, SD card boot. `scripts/fetch-pm-kernel.sh` extracts the pmbootstrap pipeline artifacts without running pmbootstrap itself: the `linux-postmarketos-allwinner` 6.17.5 kernel, the PinePhone DTBs, 431 modules and the PinePhone U-Boot from the apk packages on the postmarketOS mirror; the kernel config comes out of the IKCONFIG stream embedded in the Image, so the check runs against the configuration of the kernel that was actually built. `scripts/check-kernel-config.sh` compares 16 requirements of our userspace against that config: every mandatory one holds (VT, fbcon, ext4, evdev among them), and the single warning `CONFIG_ANDROID_BINDERFS=n` moves into the stage 4 plan as a supplement to our fragment on top of the pmOS defconfig. `scripts/pack-device-bundle.sh` assembles the bundle: a boot directory (Image, DTB, extlinux.conf), three u-boot-sunxi-with-spl variants and rootfs.ext4 packed with mke2fs -d without root, with content verified through debugfs. All three steps run in CI in the `device` job. Flashing to hardware is described in devices/pinephone/README.md as a recipe for the bring-up session and has not been executed on a device yet.

## Stage 4. GPU drivers and libhybris

On devices with a mainline driver (freedreno, panfrost, panthor) the graphics stack takes the normal path: Mesa plus DRM, no bridge layers; fbcon already shows text, the compositor comes next. On devices with a closed stack, libhybris comes in: a bridge that lets glibc binaries call Android Bionic libraries and reach the vendor GPU drivers. For HALs hidden behind Binder (camera, modem, sensors) libgbinder joins; the pmOS kernel config already carries `CONFIG_ANDROID_BINDER_IPC=y`, our config fragment supplements it with binderfs, and binderfs mounts separately.

## Stage 5. Distribution build

Rootfs build scripts (a minimal busybox and coreutils set, Afflictio userspace packages), images for the first target, flashing documentation through fastboot and SD. Pieces proven in stages 1-3: musl binary builds, cpio packing, kernel modules inside the initramfs, boot composition with modules, pmOS artifacts and rootfs.ext4 through mke2fs -d. The stage ends with an image that flashes onto the PinePhone and boots to a terminal with autologin.

## Open questions

- The first device choice is settled (PinePhone); when the target list grows, the comparison starts with the OnePlus 6, which has a mainline qcom-sdm845 kernel and wide pmOS support.
- Whether to take a ready compositor (weston, sway, labwc) or build a minimal one is decided together with stage 4, once freedreno and libhybris show which APIs are available.
- The update model (A/B partitions versus full-ota) is chosen during stage 5.
