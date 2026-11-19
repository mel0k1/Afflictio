#!/bin/sh
set -eu

: "${OUT_DIR:=out}"
MINIROOTFS_URL="${MINIROOTFS_URL:-https://dl-cdn.alpinelinux.org/alpine/v3.22/releases/aarch64/alpine-minirootfs-3.22.8-aarch64.tar.gz}"

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${REPO_ROOT}"
DEV="${OUT_DIR}/device/pinephone"
BUNDLE="${DEV}/bundle"

if ! command -v mke2fs > /dev/null 2>&1; then
  echo "mke2fs not found: install e2fsprogs"
  exit 1
fi

cargo build --release --target aarch64-unknown-linux-musl
./scripts/fetch-pm-kernel.sh

BIN_DIR="target/aarch64-unknown-linux-musl/release"
mkdir -p "${BUNDLE}/boot" "${BUNDLE}/uboot"

cp "${DEV}/boot/Image" "${DEV}/boot/sun50i-a64-pinephone-1.2.dtb" "${BUNDLE}/boot/"
mkdir -p "${BUNDLE}/uboot/pine64-pinephone"
cp "${DEV}/uboot/"*.bin "${BUNDLE}/uboot/pine64-pinephone/"
mkdir -p "${BUNDLE}/boot/extlinux"
install -m 0644 devices/pinephone/extlinux.conf "${BUNDLE}/boot/extlinux/extlinux.conf"

rm -rf "${BUNDLE}/rootfs"
mkdir -p "${BUNDLE}/rootfs"
ROOT="${BUNDLE}/rootfs"
mkdir -p "${ROOT}/bin" "${ROOT}/sbin" "${ROOT}/etc/afflictio" "${ROOT}/lib" "${ROOT}/dev" "${ROOT}/proc" "${ROOT}/sys" "${ROOT}/run" "${ROOT}/tmp"

if [ ! -f "${OUT_DIR}/minirootfs.tar.gz" ]; then
  curl -fL --max-time 300 "${MINIROOTFS_URL}" -o "${OUT_DIR}/minirootfs.tar.gz"
fi
tar -xzf "${OUT_DIR}/minirootfs.tar.gz" -C "${DEV}" ./bin/busybox
install -m 0755 "${DEV}/bin/busybox" "${ROOT}/bin/busybox"
for applet in sh mount umount mkdir mknod sleep killall modprobe grep ls cat ln sed awk wc cmp dd head tail find cp rm mv chmod uname sync ps; do
  ln -sf busybox "${ROOT}/bin/${applet}"
done

install -m 0755 "${BIN_DIR}/liinit" "${ROOT}/sbin/liinit"
install -m 0755 "${BIN_DIR}/tiosessd" "${ROOT}/bin/tiosessd"
install -m 0755 "${BIN_DIR}/tioctl" "${ROOT}/bin/tioctl"
install -m 0644 devices/pinephone/etc/init.toml "${ROOT}/etc/afflictio/init.toml"
install -m 0644 kernel/initramfs/session.toml "${ROOT}/etc/afflictio/session.toml"
cp -r "${DEV}/modules" "${ROOT}/lib/"

rm -f "${BUNDLE}/rootfs.ext4"
mke2fs -q -F -t ext4 -d "${ROOT}" -L afflictio "${BUNDLE}/rootfs.ext4" 512M

binaries="$(debugfs -R "ls /bin" "${BUNDLE}/rootfs.ext4" 2>/dev/null | grep -oE "tiosessd|tioctl|busybox" | sort -u | wc -l)"
sbin="$(debugfs -R "cat /sbin/liinit" "${BUNDLE}/rootfs.ext4" 2>/dev/null | wc -c)"
if [ "${binaries}" -lt 3 ] || [ "${sbin}" -lt 100000 ]; then
  echo "rootfs.ext4 check failed: binaries missing (${binaries}/3 in /bin, liinit ${sbin} bytes)"
  exit 1
fi
if ! debugfs -R "cat /etc/afflictio/session.toml" "${BUNDLE}/rootfs.ext4" 2>/dev/null | grep -q "autologin"; then
  echo "rootfs.ext4 check failed: session config missing"
  exit 1
fi
if [ ! -s "${BUNDLE}/rootfs.ext4" ]; then
  echo "rootfs.ext4 is empty"
  exit 1
fi

kver="$(ls "${ROOT}/lib/modules")"
modules_count="$(debugfs -R "ls -l /lib/modules/${kver}/kernel/drivers" "${BUNDLE}/rootfs.ext4" 2>/dev/null | wc -l)"
image_size="$(wc -c < "${BUNDLE}/boot/Image")"
ext4_size="$(wc -c < "${BUNDLE}/rootfs.ext4")"

echo "pinephone bundle ready:"
echo "  boot: Image ${image_size} bytes + dtb + extlinux.conf"
echo "  uboot: u-boot-sunxi-with-spl.bin"
echo "  rootfs.ext4: ${ext4_size} bytes, /bin binaries: ${binaries}, modules dir entries: ${modules_count}"
echo "  rootfs tree: $(find "${ROOT}" -type f | wc -l) files"
