#!/bin/sh
set -eu

: "${OUT_DIR:=out}"

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${REPO_ROOT}"
mkdir -p "${OUT_DIR}"

cargo build --release --target aarch64-unknown-linux-musl
./scripts/prepare-kernel.sh
./scripts/mkinitramfs.sh

LOG="${OUT_DIR}/serial.log"
MON="${OUT_DIR}/monitor.sock"
rm -f "${LOG}" "${MON}"

qemu-system-aarch64 \
  -machine virt -cpu cortex-a57 -m 512M -nodefaults \
  -device virtio-gpu-pci \
  -device virtio-keyboard-pci \
  -kernel "${OUT_DIR}/kernel/Image" \
  -initrd "${OUT_DIR}/initramfs.cpio.gz" \
  -append "console=ttyAMA0 panic=-1" \
  -display none -serial "file:${LOG}" \
  -qmp "unix:${MON},server,nowait" \
  -no-reboot > "${LOG}.qemu" 2>&1 &
QPID=$!

qemu_alive() {
  kill -0 "${QPID}" 2>/dev/null
}

wait_log() {
  marker=$1
  limit=$2
  n=0
  while [ $n -lt "${limit}" ]; do
    if grep -q "${marker}" "${LOG}" 2>/dev/null; then
      return 0
    fi
    if ! qemu_alive; then
      return 1
    fi
    sleep 1
    n=$((n+1))
  done
  return 1
}

try_switch() {
  marker=$1
  key=$2
  n=0
  while [ $n -lt 12 ]; do
    if grep -q "${marker}" "${LOG}" 2>/dev/null; then
      return 0
    fi
    python3 scripts/qemu-monitor.py "${MON}" key ctrl alt "${key}" || true
    sleep 3
    n=$((n+1))
  done
  grep -q "${marker}" "${LOG}" 2>/dev/null
}

trap 'kill "${QPID}" 2>/dev/null || true' EXIT

status_fail() {
  echo "$1"
  grep -a "SMOKE" "${LOG}" || true
  tail -5 "${LOG}" 2>/dev/null || true
  exit 1
}

wait_log "SMOKE_READY" 180 || status_fail "smoke failed: sessions or keyboard not ready"
try_switch "SMOKE_SWITCH_VT1_OK" f1 || status_fail "smoke failed: keyboard switch to vt1"
try_switch "SMOKE_SWITCH_VT2_OK" f2 || status_fail "smoke failed: keyboard switch to vt2"
wait_log "SMOKE_FRAMES_DIFFER" 10 || status_fail "smoke failed: vt frames identical"
wait_log "SMOKE_KMS_FLIPS_OK" 60 || status_fail "smoke failed: kms compositor flips"
wait_log "SMOKE_RESPAWN_OK" 120 || status_fail "smoke failed: respawn did not recover sessiond"

n=0
while qemu_alive && [ $n -lt 120 ]; do
  sleep 1
  n=$((n+1))
done
rc=0
if qemu_alive; then
  kill "${QPID}" 2>/dev/null || true
fi
wait "${QPID}" || rc=$?
trap - EXIT

echo "qemu exit: ${rc}"
grep -a "SMOKE" "${LOG}" || true

if ! grep -q "SMOKE_SWITCH_VT1_OK" "${LOG}"; then
  echo "smoke failed: keyboard switch to vt1 missing"
  exit 1
fi
if ! grep -q "SMOKE_SWITCH_VT2_OK" "${LOG}"; then
  echo "smoke failed: keyboard switch to vt2 missing"
  exit 1
fi
if ! grep -q "SMOKE_FRAMES_DIFFER" "${LOG}"; then
  echo "smoke failed: vt frames identical"
  exit 1
fi
if ! grep -q "SMOKE_KMS_FLIPS_OK" "${LOG}"; then
  echo "smoke failed: kms compositor flips missing"
  exit 1
fi
if ! grep -q "SMOKE_RESPAWN_OK" "${LOG}"; then
  echo "smoke failed: respawn did not recover sessiond"
  exit 1
fi
if [ "${rc}" != "0" ]; then
  echo "smoke failed: qemu exit code ${rc}"
  exit 1
fi
echo "smoke passed"
