#!/bin/sh
set -eu

: "${OUT_DIR:=out}"
KERNEL_URL="${KERNEL_URL:-https://dl-cdn.alpinelinux.org/alpine/v3.22/releases/aarch64/netboot/vmlinuz-virt}"
QEMU_TIMEOUT="${QEMU_TIMEOUT:-240}"

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${REPO_ROOT}"
mkdir -p "${OUT_DIR}"

KERNEL="${OUT_DIR}/vmlinuz-virt"
if [ ! -f "${KERNEL}" ]; then
  curl -fL "${KERNEL_URL}" -o "${KERNEL}"
fi

IMAGE="${KERNEL}"
if [ "$(head -c 2 "${KERNEL}")" = "MZ" ]; then
  gz_magic="$(printf '\037\213\010')"
  off="$(grep -abo "$gz_magic" "${KERNEL}" | head -1 | cut -d: -f1)"
  tail -c "+$((off + 1))" "${KERNEL}" | gzip -d > "${OUT_DIR}/Image" || true
  if [ ! -s "${OUT_DIR}/Image" ]; then
    echo "zboot payload extraction failed"
    exit 1
  fi
  IMAGE="${OUT_DIR}/Image"
fi

cargo build --release --target aarch64-unknown-linux-musl
./scripts/mkinitramfs.sh

LOG="${OUT_DIR}/serial.log"
APPEND="console=ttyAMA0 panic=-1"
rc=0
if [ -n "${QEMU_DATA_DIR:-}" ]; then
  timeout "${QEMU_TIMEOUT}" qemu-system-aarch64 -L "${QEMU_DATA_DIR}" \
    -machine virt -cpu cortex-a57 -m 512M -nodefaults \
    -kernel "${IMAGE}" -initrd "${OUT_DIR}/initramfs.cpio.gz" \
    -append "${APPEND}" -display none -serial "file:${LOG}" -no-reboot > "${LOG}.qemu" 2>&1 || rc=$?
else
  timeout "${QEMU_TIMEOUT}" qemu-system-aarch64 \
    -machine virt -cpu cortex-a57 -m 512M -nodefaults \
    -kernel "${IMAGE}" -initrd "${OUT_DIR}/initramfs.cpio.gz" \
    -append "${APPEND}" -display none -serial "file:${LOG}" -no-reboot > "${LOG}.qemu" 2>&1 || rc=$?
fi

echo "qemu exit: ${rc}"
grep -E "SMOKE" "${LOG}" || true

if ! grep -q "SMOKE_IPC_OK" "${LOG}"; then
  echo "smoke failed: sessiond IPC unreachable"
  exit 1
fi
if ! grep -q "SMOKE_RESPAWN_OK" "${LOG}"; then
  echo "smoke failed: respawn did not recover sessiond"
  exit 1
fi
if ! grep -q "SMOKE_SESSIONS_OK" "${LOG}"; then
  echo "smoke warning: autologin session absent"
fi
if [ "${rc}" != "0" ]; then
  echo "smoke failed: qemu exit code ${rc}"
  exit 1
fi
echo "smoke passed"
