#!/bin/sh
set -eu

: "${OUT_DIR:=out}"

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${REPO_ROOT}"

./scripts/build-rootfs.sh
./scripts/mkrootfs-initramfs.sh

LOG="${OUT_DIR}/rootfs-serial.log"
MON="${OUT_DIR}/rootfs-monitor.sock"
MODEM_SOCK="${OUT_DIR}/modem.sock"
rm -f "${LOG}" "${MON}" "${MODEM_SOCK}"

DTB_PROBE="${OUT_DIR}/virt-dtb.bin"
MODEM=0
if qemu-system-aarch64 \
  -machine virt,dumpdtb="${DTB_PROBE}" -cpu cortex-a57 -m 512M -nodefaults \
  -serial null -serial null -display none > /dev/null 2>&1 &&
  strings "${DTB_PROBE}" | grep -q "pl011@9040000"; then
  MODEM=1
fi
rm -f "${DTB_PROBE}"

MODEM_PID=""
if [ "${MODEM}" = "1" ]; then
  python3 scripts/modem-emu.py "${MODEM_SOCK}" > "${OUT_DIR}/modem-emu.log" 2>&1 &
  MODEM_PID=$!
fi

if [ "${MODEM}" = "1" ]; then
  qemu-system-aarch64 \
    -machine virt -cpu cortex-a57 -m 512M -nodefaults \
    -device virtio-gpu-pci \
    -device virtio-keyboard-pci \
    -drive file="${OUT_DIR}/rootfs.ext4",if=virtio,format=raw \
    -kernel "${OUT_DIR}/kernel/Image" \
    -initrd "${OUT_DIR}/rootfs-initramfs.cpio.gz" \
    -append "console=ttyAMA0 panic=-1" \
    -display none -serial "file:${LOG}" \
    -serial "unix:${MODEM_SOCK},server=on,wait=off" \
    -qmp "unix:${MON},server,nowait" \
    -no-reboot > "${LOG}.qemu" 2>&1 &
else
  qemu-system-aarch64 \
    -machine virt -cpu cortex-a57 -m 512M -nodefaults \
    -device virtio-gpu-pci \
    -device virtio-keyboard-pci \
    -drive file="${OUT_DIR}/rootfs.ext4",if=virtio,format=raw \
    -kernel "${OUT_DIR}/kernel/Image" \
    -initrd "${OUT_DIR}/rootfs-initramfs.cpio.gz" \
    -append "console=ttyAMA0 panic=-1" \
    -display none -serial "file:${LOG}" \
    -qmp "unix:${MON},server,nowait" \
    -no-reboot > "${LOG}.qemu" 2>&1 &
fi
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

trap 'kill "${QPID}" "${MODEM_PID}" 2>/dev/null || true' EXIT

status_fail() {
  echo "$1"
  grep -a "SMOKE" "${LOG}" || true
  tail -5 "${LOG}" 2>/dev/null || true
  exit 1
}

wait_log "SMOKE_READY" 240 || status_fail "rootfs smoke failed: sessions or keyboard not ready"
try_switch "SMOKE_SWITCH_VT1_OK" f1 || status_fail "rootfs smoke failed: keyboard switch to vt1"
try_switch "SMOKE_SWITCH_VT2_OK" f2 || status_fail "rootfs smoke failed: keyboard switch to vt2"
wait_log "SMOKE_FRAMES_DIFFER" 10 || status_fail "rootfs smoke failed: vt frames identical"
wait_log "SMOKE_UID_OK" 30 || status_fail "rootfs smoke failed: sessions not running as uid 1000"
wait_log "SMOKE_KMS_FLIPS_OK" 60 || status_fail "rootfs smoke failed: kms compositor flips"
wait_log "SMOKE_RESPAWN_OK" 120 || status_fail "rootfs smoke failed: respawn did not recover sessiond"
if [ "${MODEM}" = "1" ]; then
  wait_log "SMOKE_MODEM_AT_OK" 60 || status_fail "rootfs smoke failed: modem probe over ttyAMA1"
fi

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
  echo "rootfs smoke failed: keyboard switch to vt1 missing"
  exit 1
fi
if ! grep -q "SMOKE_SWITCH_VT2_OK" "${LOG}"; then
  echo "rootfs smoke failed: keyboard switch to vt2 missing"
  exit 1
fi
if ! grep -q "SMOKE_FRAMES_DIFFER" "${LOG}"; then
  echo "rootfs smoke failed: vt frames identical"
  exit 1
fi
if ! grep -q "SMOKE_UID_OK" "${LOG}"; then
  echo "rootfs smoke failed: sessions not running as uid 1000"
  exit 1
fi
if ! grep -q "SMOKE_KMS_FLIPS_OK" "${LOG}"; then
  echo "rootfs smoke failed: kms compositor flips missing"
  exit 1
fi
if ! grep -q "SMOKE_RESPAWN_OK" "${LOG}"; then
  echo "rootfs smoke failed: respawn did not recover sessiond"
  exit 1
fi
if [ "${rc}" != "0" ]; then
  echo "rootfs smoke failed: qemu exit code ${rc}"
  exit 1
fi
if [ "${MODEM}" = "1" ]; then
  if ! grep -q "SMOKE_MODEM_AT_OK" "${LOG}"; then
    echo "rootfs smoke failed: modem probe over ttyAMA1 missing"
    exit 1
  fi
else
  echo "modem check skipped: this qemu virt build has no second pl011"
fi
echo "rootfs smoke passed"
echo "SMOKE_PASSED"
