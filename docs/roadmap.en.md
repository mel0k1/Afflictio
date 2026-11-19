# Afflictio Roadmap

## Where the project is

Stage 1 is done: the skeleton boots in QEMU on ARM64 with one script, `scripts/qemu-smoke.sh`, and the same scenario runs in CI. Inside the machine liinit mounts the filesystems and starts services, tiosessd brings up an autologin session on a VT, tioctl talks over the IPC, the supervisor restarts a killed daemon, and SIGTERM ends in a clean poweroff. The skeleton is tagged v0.1.0. There is still a long way to an installable phone distribution, and it is more honest to name the steps that separate the skeleton from a working system.

## Stage 1. Skeleton check in QEMU (done)

Booting a kernel with initramfs in `qemu-system-aarch64 -machine virt` and confirming that init mounts the filesystems, starts services, reacts to signals and shuts the machine down. The test uses the `linux-virt` kernel from the Alpine netboot repository: it is built for emulation and carries everything the virt machine needs. The kernel-from-source build script with the config fragment stays in scripts/build-kernel.sh for real devices. The check joined CI as the `qemu` job.

## Stage 2. Graphics and sessions

A terminal-only scenario: text output through `simpledrm` or the framebuffer, keyboard attached through the HAL, VT switching driven by the keyboard. A session gets its TTY and controlling terminal, already described in the architecture; the stage 1 QEMU run showed that VT infrastructure works on the virt kernel. Exit criterion: two sessions switch by keyboard with no commands typed.

## Stage 3. Devices through postmarketOS

Real phones need DTS and vendor drivers that mainline lacks. The chosen path reuses existing work: postmarketOS device patches (pmbootstrap provides both kernel and rootfs). Plan: pick a first target from the widely supported pmOS devices (OnePlus 6 or PinePhone, for example), extract the kernel and DTS from the pmbootstrap pipeline and build our userspace on top. The kernel fragment in kernel/ becomes an addition to their defconfig, not a replacement.

## Stage 4. GPU drivers and libhybris

On devices with a mainline driver (freedreno, panfrost, panthor) the graphics stack takes the normal path: Mesa plus DRM, no bridge layers. On devices with a closed stack, libhybris comes in: a bridge that lets glibc binaries call Android Bionic libraries and reach the vendor GPU drivers. For HALs hidden behind Binder (camera, modem, sensors) libgbinder joins; the kernel config already enables `CONFIG_ANDROID_BINDER_IPC` and `CONFIG_ANDROID_BINDERFS`, and binderfs mounts separately.

## Stage 5. Distribution build

Rootfs build scripts (a minimal busybox and coreutils set, Afflictio userspace packages), images for the first target, flashing documentation through fastboot. The stage 1 initramfs work already produced reusable pieces: musl binary builds, cpio packing, boot composition. The stage ends with an image that flashes onto a supported device and boots to a terminal with autologin.

## Open questions

- The first device choice depends on pmOS kernel availability and our debugging budget for DTS.
- Whether to take a ready compositor (weston, sway, labwc) or build a minimal one is decided together with stage 2.
- The update model (A/B partitions versus full-ota) is chosen during stage 5.
