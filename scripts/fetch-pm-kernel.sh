#!/bin/sh
set -eu

: "${OUT_DIR:=out}"
PMOS_BRANCH="${PMOS_BRANCH:-v25.12}"
PMOS_KERNEL_APK="${PMOS_KERNEL_APK:-linux-postmarketos-allwinner-6.17.5_git20251026-r2.apk}"
PMOS_KERNEL_PACKAGE="${PMOS_KERNEL_PACKAGE:-linux-postmarketos-allwinner}"
PMOS_UBOOT_APK="${PMOS_UBOOT_APK:-u-boot-pinephone-2023.01-r4.apk}"
PMOS_UBOOT_PACKAGE="${PMOS_UBOOT_PACKAGE:-u-boot-pinephone}"

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${REPO_ROOT}"
DEV="${OUT_DIR}/device/pinephone"
mkdir -p "${DEV}"

resolve_apk() {
  package=$1
  pinned=$2
  if curl -fsIL --max-time 30 "https://mirror.postmarketos.org/postmarketos/${PMOS_BRANCH}/aarch64/${pinned}" > /dev/null 2>&1; then
    printf '%s\n' "${pinned}"
    return 0
  fi
  curl -fsL --max-time 60 "https://mirror.postmarketos.org/postmarketos/${PMOS_BRANCH}/aarch64/APKINDEX.tar.gz" | tar -xzO APKINDEX > "${DEV}/APKINDEX" 2>/dev/null
  version="$(awk -v pkg="P:${package}" '
    $0 == pkg { found=1 }
    found && /^V:/ { print substr($0, 3); exit }
    found && /^$/ { exit }
  ' "${DEV}/APKINDEX")"
  if [ -z "${version}" ]; then
    echo "package ${package} not found in APKINDEX"
    exit 1
  fi
  echo "pinned ${pinned} unavailable, resolving ${package} to ${version}"
  printf '%s_%s.apk\n' "${package}" "${version}"
}

download_apk() {
  apk_name=$1
  target=$2
  if [ ! -f "${target}" ]; then
    curl -fL --max-time 600 "https://mirror.postmarketos.org/postmarketos/${PMOS_BRANCH}/aarch64/${apk_name}" -o "${target}"
  fi
}

kernel_apk="$(resolve_apk "${PMOS_KERNEL_PACKAGE}" "${PMOS_KERNEL_APK}")"
download_apk "${kernel_apk}" "${DEV}/kernel.apk"

uboot_apk="$(resolve_apk "${PMOS_UBOOT_PACKAGE}" "${PMOS_UBOOT_APK}")"
download_apk "${uboot_apk}" "${DEV}/uboot.apk"

rm -rf "${DEV}/extract"
mkdir -p "${DEV}/extract"
tar -xzf "${DEV}/kernel.apk" -C "${DEV}/extract" boot/vmlinuz "boot/dtbs/allwinner" lib/modules 2>/dev/null || true
tar -xzf "${DEV}/uboot.apk" -C "${DEV}/extract" 2>/dev/null || true

mkdir -p "${DEV}/boot" "${DEV}/modules" "${DEV}/uboot"
cp "${DEV}/extract/boot/vmlinuz" "${DEV}/boot/Image"
for board in sun50i-a64-pinephone-1.1 sun50i-a64-pinephone-1.2 sun50i-a64-pinephone-1.2b; do
  cp "${DEV}/extract/boot/dtbs/allwinner/${board}.dtb" "${DEV}/boot/"
done
cp -r "${DEV}/extract/lib/modules/"* "${DEV}/modules/"
uboot_bins="$(find "${DEV}/extract" -name "u-boot-sunxi-with-spl-*.bin")"
if [ -z "${uboot_bins}" ]; then
  echo "u-boot-sunxi-with-spl binaries not found in ${uboot_apk}"
  exit 1
fi
for uboot_bin in ${uboot_bins}; do
  cp "${uboot_bin}" "${DEV}/uboot/"
done

off="$(grep -abo "IKCFG_ST" "${DEV}/boot/Image" | head -1 | cut -d: -f1)"
if [ -z "${off}" ]; then
  echo "embedded kernel config (IKCFG_ST) not found in Image"
  exit 1
fi
tail -c "+$((off + 9))" "${DEV}/boot/Image" | gunzip 2>/dev/null > "${DEV}/kernel.config" || true
if ! grep -q "^CONFIG_" "${DEV}/kernel.config" 2>/dev/null; then
  tail -c "+$((off + 9))" "${DEV}/boot/Image" | zstd -d 2>/dev/null > "${DEV}/kernel.config" || true
fi
if ! grep -q "^CONFIG_" "${DEV}/kernel.config" 2>/dev/null; then
  echo "failed to decompress embedded kernel config"
  exit 1
fi

kver="$(ls "${DEV}/modules")"
image_size="$(wc -c < "${DEV}/boot/Image")"
module_count="$(find "${DEV}/modules/${kver}" -name "*.ko*" | wc -l)"
dtb_magic="$(head -c 4 "${DEV}/boot/sun50i-a64-pinephone-1.2.dtb" | od -A n -t x1 | tr -d ' \n')"
if [ "${image_size}" -lt 5000000 ]; then
  echo "kernel Image too small: ${image_size} bytes"
  exit 1
fi
if [ "${dtb_magic}" != "d00dfeed" ]; then
  echo "pinephone dtb magic check failed: ${dtb_magic}"
  exit 1
fi

echo "pinephone kernel artifacts ready:"
echo "  kernel apk: ${kernel_apk} (kver ${kver})"
echo "  image: ${image_size} bytes"
echo "  dtbs: sun50i-a64-pinephone-1.1 1.2 1.2b"
echo "  modules: ${module_count} files (kver ${kver})"
echo "  config: $(grep -c "^CONFIG_" "${DEV}/kernel.config") options"
