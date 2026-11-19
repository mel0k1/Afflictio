#!/bin/sh
set -eu

: "${OUT_DIR:=out}"
MINIROOTFS_URL="${MINIROOTFS_URL:-https://dl-cdn.alpinelinux.org/alpine/v3.22/releases/aarch64/alpine-minirootfs-3.22.8-aarch64.tar.gz}"

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="${OUT_DIR}/initramfs"
BIN_DIR="target/aarch64-unknown-linux-musl/release"

cd "${REPO_ROOT}"
mkdir -p "${OUT_DIR}" "${ROOT}"

if [ ! -f "${BIN_DIR}/liinit" ]; then
  cargo build --release --target aarch64-unknown-linux-musl
fi

./scripts/prepare-kernel.sh

if [ ! -f "${ROOT}/bin/busybox" ]; then
  curl -fL "${MINIROOTFS_URL}" -o "${OUT_DIR}/minirootfs.tar.gz"
  tar -xzf "${OUT_DIR}/minirootfs.tar.gz" -C "${ROOT}"
fi

mkdir -p "${ROOT}/etc/afflictio" "${ROOT}/lib"
install -m 0755 "${BIN_DIR}/liinit" "${ROOT}/bin/liinit"
install -m 0755 "${BIN_DIR}/tiosessd" "${ROOT}/bin/tiosessd"
install -m 0755 "${BIN_DIR}/tioctl" "${ROOT}/bin/tioctl"
install -m 0755 kernel/initramfs/init "${ROOT}/init"
install -m 0755 kernel/initramfs/probe.sh "${ROOT}/bin/probe.sh"
install -m 0644 kernel/initramfs/init.toml "${ROOT}/etc/afflictio/init.toml"
install -m 0644 kernel/initramfs/session.toml "${ROOT}/etc/afflictio/session.toml"
cp -r "${OUT_DIR}/kernel/lib/modules" "${ROOT}/lib/"

cd "${ROOT}"
find . -print0 | cpio -0 -o -H newc 2>/dev/null | gzip -9 > "${REPO_ROOT}/${OUT_DIR}/initramfs.cpio.gz"
ls -la "${REPO_ROOT}/${OUT_DIR}/initramfs.cpio.gz"
