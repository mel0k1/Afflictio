#!/bin/sh
set -eu

: "${LINUX_VERSION:=6.12}"
: "${CROSS_COMPILE:=aarch64-linux-gnu-}"
: "${JOBS:=$(nproc)}"
: "${OUTPUT_DIR:=out}"

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC_DIR="${OUTPUT_DIR}/linux-${LINUX_VERSION}"
ARCHIVE="${OUTPUT_DIR}/linux-${LINUX_VERSION}.tar.xz"

mkdir -p "${OUTPUT_DIR}"

if [ ! -d "${SRC_DIR}" ]; then
  curl -fL "https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-${LINUX_VERSION}.tar.xz" -o "${ARCHIVE}"
  tar -xf "${ARCHIVE}" -C "${OUTPUT_DIR}"
fi

cd "${SRC_DIR}"
make ARCH=arm64 CROSS_COMPILE="${CROSS_COMPILE}" defconfig
scripts/kconfig/merge_config.sh -m .config "${REPO_ROOT}/kernel/configs/afflictio.fragment"
make ARCH=arm64 CROSS_COMPILE="${CROSS_COMPILE}" olddefconfig
make ARCH=arm64 CROSS_COMPILE="${CROSS_COMPILE}" -j"${JOBS}" Image dtbs modules
