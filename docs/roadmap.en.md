# Afflictio Roadmap

## Where the project is

Stages 1 and 2 are done (tags v0.1.0 and v0.2.0). The skeleton boots in QEMU on ARM64 with one script, `scripts/qemu-smoke.sh`, and the same scenario runs in CI: liinit mounts the filesystems and starts services, tiosessd brings up two autologin sessions on VT1 and VT2, the keyboard switches consoles through the HAL with Ctrl+Alt+Fn, session text really lands on the framebuffer through virtio-gpu, the supervisor restarts a killed daemon, and SIGTERM ends in a clean poweroff. There is still a long way to an installable phone distribution, and it is more honest to name the steps that separate the skeleton from a working system.

## Stage 1. Skeleton check in QEMU (done, v0.1.0)

Booting a kernel with initramfs in `qemu-system-aarch64 -machine virt` and confirming that init mounts the filesystems, starts services, reacts to signals and shuts the machine down. The test uses the `linux-virt` kernel from the Alpine repository: it is built for emulation and carries everything the virt machine needs. The kernel-from-source build script with the config fragment stays in scripts/build-kernel.sh for real devices. The check joined CI as the `qemu` job.

## Stage 2. Graphics and sessions (done, v0.2.0)

The terminal-only scenario reached an automated check. Text output: fbcon is built into the kernel, the `virtio-gpu` driver loads as a module from the kernel apk package; sessions print text on their VTs and the content of `/dev/fb0` compares byte by byte between the two consoles. Keyboard attachment: `virtio-keyboard-pci` in QEMU, the HAL reads evdev events, `ComboTracker` recognizes Ctrl+Alt+Fn, and a daemon thread rescans devices and sends switches into the main loop. VT switching by keyboard is verified with the QMP send-key command: the `SMOKE_SWITCH_VT1_OK` and `SMOKE_SWITCH_VT2_OK` markers confirm that both sessions switch by keyboard with no commands typed. Along the way a real bug got fixed: the session tty opened read-only, so all session output was silently lost.

## Stage 3. Devices through postmarketOS

Real phones need DTS and vendor drivers that mainline lacks. The chosen path reuses existing work: postmarketOS device patches (pmbootstrap provides both kernel and rootfs). Plan: pick a first target from the widely supported pmOS devices (OnePlus 6 or PinePhone, for example), extract the kernel and DTS from the pmbootstrap pipeline and build our userspace on top. The kernel fragment in kernel/ becomes an addition to their defconfig, not a replacement. The stage 2 experience with kernel modules in the initramfs transfers directly: pmOS kernels carry their own module sets, and the apk-extraction scheme swaps for pmbootstrap artifacts.

## Stage 4. GPU drivers and libhybris

On devices with a mainline driver (freedreno, panfrost, panthor) the graphics stack takes the normal path: Mesa plus DRM, no bridge layers; fbcon already shows text, the compositor comes next. On devices with a closed stack, libhybris comes in: a bridge that lets glibc binaries call Android Bionic libraries and reach the vendor GPU drivers. For HALs hidden behind Binder (camera, modem, sensors) libgbinder joins; the kernel config already enables `CONFIG_ANDROID_BINDER_IPC` and `CONFIG_ANDROID_BINDERFS`, and binderfs mounts separately.

## Stage 5. Distribution build

Rootfs build scripts (a minimal busybox and coreutils set, Afflictio userspace packages), images for the first target, flashing documentation through fastboot. Pieces proven in stages 1 and 2: musl binary builds, cpio packing, kernel modules inside the initramfs, boot composition with modules. The stage ends with an image that flashes onto a supported device and boots to a terminal with autologin.

## Open questions

- The first device choice depends on pmOS kernel availability and our debugging budget for DTS.
- Whether to take a ready compositor (weston, sway, labwc) or build a minimal one is decided together with stage 4, once freedreno and libhybris show which APIs are available.
- The update model (A/B partitions versus full-ota) is chosen during stage 5.
