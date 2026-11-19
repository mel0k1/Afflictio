#!/bin/sh
set -eu

: "${OUT_DIR:=out}"
KERNEL_APK_URL="${KERNEL_APK_URL:-https://dl-cdn.alpinelinux.org/alpine/v3.22/main/aarch64/linux-virt-6.12.118-r0.apk}"

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${REPO_ROOT}"
mkdir -p "${OUT_DIR}/kernel"

APK="${OUT_DIR}/kernel/linux-virt.apk"
if [ ! -f "${APK}" ]; then
  curl -fL "${KERNEL_APK_URL}" -o "${APK}"
fi

if [ ! -f "${OUT_DIR}/kernel/vmlinuz-virt" ]; then
  tar -xzf "${APK}" -C "${OUT_DIR}/kernel" --strip-components=1 boot/vmlinuz-virt 2>/dev/null || true
fi
if [ ! -f "${OUT_DIR}/kernel/vmlinuz-virt" ]; then
  echo "kernel extraction failed"
  exit 1
fi

ver="$(cat "${OUT_DIR}/kernel/modules.version" 2>/dev/null || true)"
if [ -z "${ver}" ]; then
  ver="$(tar -tzf "${APK}" 2>/dev/null | grep -oE '^lib/modules/[^/]+/' | head -1 | cut -d/ -f3)"
fi
if [ -z "${ver}" ]; then
  echo "module directory not found in apk"
  exit 1
fi
printf '%s\n' "${ver}" > "${OUT_DIR}/kernel/modules.version"

MODROOT="${OUT_DIR}/kernel/lib/modules/${ver}"
if [ ! -f "${MODROOT}/kernel/drivers/input/evdev.ko.gz" ]; then
  rm -rf "${MODROOT}"
  tar -xzf "${APK}" -C "${OUT_DIR}/kernel" \
    "lib/modules/${ver}/modules.dep" \
    "lib/modules/${ver}/kernel/drivers/gpu" \
    "lib/modules/${ver}/kernel/drivers/virtio" \
    "lib/modules/${ver}/kernel/drivers/video" \
    "lib/modules/${ver}/kernel/drivers/i2c" \
    "lib/modules/${ver}/kernel/drivers/input" 2>/dev/null || true
fi
if [ ! -f "${MODROOT}/modules.dep" ] || [ ! -f "${MODROOT}/kernel/drivers/input/evdev.ko.gz" ]; then
  echo "module extraction failed"
  exit 1
fi

IMAGE="${OUT_DIR}/kernel/Image"
if [ ! -f "${IMAGE}" ]; then
  if [ "$(head -c 2 "${OUT_DIR}/kernel/vmlinuz-virt")" = "MZ" ]; then
    gz_magic="$(printf '\037\213\010')"
    off="$(grep -abo "$gz_magic" "${OUT_DIR}/kernel/vmlinuz-virt" | head -1 | cut -d: -f1)"
    tail -c "+$((off + 1))" "${OUT_DIR}/kernel/vmlinuz-virt" | gzip -d > "${IMAGE}" || true
    if [ ! -s "${IMAGE}" ]; then
      echo "zboot payload extraction failed"
      exit 1
    fi
  else
    cp "${OUT_DIR}/kernel/vmlinuz-virt" "${IMAGE}"
  fi
fi
echo "kernel ready: ${IMAGE}"
