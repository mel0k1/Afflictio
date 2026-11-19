# Afflictio Roadmap

## Where the project is

The skeleton: init as PID 1 with a service supervisor, a HAL that reads evdev over epoll, a session daemon with an IPC server, the `afflictioctl` client, the kernel config fragment and the build script. Everything compiles for x86_64 and passes `cargo check` for AArch64. There is still a long way to an installable phone distribution, and it is more honest to name the steps that separate the skeleton from a working system.

## Stage 1. Skeleton check in QEMU

Goal: boot a kernel with initramfs in QEMU (`qemu-system-aarch64 -M virt`) and confirm that init mounts the filesystems, starts services and reacts to signals. This includes an initramfs image with the compiled crates and running sessiond with autologin on a virtual terminal. The stage ends with a tag, after which the QEMU run joins CI.

## Stage 2. Graphics and sessions

A terminal-only scenario: text output through `simpledrm` or the framebuffer in the style of `kmscon`, keyboard attached through the HAL, VT switching. A session gets its TTY and controlling terminal, already described in the architecture. Exit criterion: on a device or in QEMU two sessions switch by keyboard.

## Stage 3. Devices through postmarketOS

Real phones need DTS and vendor drivers that mainline lacks. The chosen path reuses existing work: postmarketOS device patches (pmbootstrap provides both kernel and rootfs). Plan: pick a first target from the widely supported pmOS devices (OnePlus 6 or PinePhone, for example), extract the kernel and DTS from the pmbootstrap pipeline and build our userspace on top. The kernel fragment in kernel/ becomes an addition to their defconfig, not a replacement.

## Stage 4. GPU drivers and libhybris

On devices with a mainline driver (freedreno, panfrost, panthor) the graphics stack takes the normal path: Mesa plus DRM, no bridge layers. On devices with a closed stack, libhybris comes in: a bridge that lets glibc binaries call Android Bionic libraries and reach the vendor GPU drivers. For HALs hidden behind Binder (camera, modem, sensors) libgbinder joins; the kernel config already enables `CONFIG_ANDROID_BINDER_IPC` and `CONFIG_ANDROID_BINDERFS`, and binderfs mounts separately.

## Stage 5. Distribution build

Rootfs build scripts (a minimal busybox and coreutils set, Afflictio userspace packages), images for the first target, flashing documentation through fastboot. The stage ends with an image that flashes onto a supported device and boots to a terminal with autologin.

## Open questions

- The first device choice depends on pmOS kernel availability and our debugging budget for DTS.
- Whether to take a ready compositor (weston, sway, labwc) or build a minimal one is decided together with stage 2.
- The update model (A/B partitions versus full-ota) is chosen during stage 5.
