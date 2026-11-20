#!/bin/sh
exec >/dev/console 2>&1

active_vt() {
  /bin/tioctl status 2>/dev/null | grep -o '"active_vt": *[0-9]*' | sed 's/.*: *//'
}

uuid_for() {
  awk -v n="$1" '
    /^[[:space:]]*name[[:space:]]*=/ { split($0, a, "\""); m = (a[2] == n) }
    m && /^[[:space:]]*uuid[[:space:]]*=/ { split($0, b, "\""); print b[2]; exit }
  ' /etc/afflictio/slots.toml
}

if grep -q 'afflictio.abtest=1' /proc/cmdline; then
  uuid_b=$(uuid_for b)
  active=$(grep -o 'root=UUID=[0-9a-fA-F-]*' /proc/cmdline | sed 's/root=UUID=//')
  if [ "${active}" = "${uuid_b}" ]; then
    n=0
    boot_ok=0
    while [ $n -lt 30 ]; do
      if [ -f /var/lib/afflictio/boot-ok ]; then
        boot_ok=1
        break
      fi
      sleep 1
      n=$((n+1))
    done
    if [ "${boot_ok}" != "1" ]; then
      echo SMOKE_AB_BOOT_OK_MISSING
      kill -TERM 1
      exit 1
    fi
    out=$(/bin/liota status) || { echo SMOKE_AB_BOOT_B_FAIL; kill -TERM 1; exit 1; }
    echo "${out}"
    if ! printf '%s\n' "${out}" | grep -q '"active":"b"'; then
      echo SMOKE_AB_BOOT_B_FAIL
      kill -TERM 1
      exit 1
    fi
    echo SMOKE_AB_BOOT_B_OK
    kill -TERM 1
    exit 0
  fi
fi

n=0
ready=0
while [ $n -lt 90 ]; do
  out=$(/bin/tioctl status 2>/dev/null)
  sessions=$(printf '%s\n' "$out" | grep -o '"user": "mel0k1"' | wc -l)
  keys=$(printf '%s\n' "$out" | grep -o '"keyboard_devices": *[0-9]*' | sed 's/.*: *//')
  if [ "${sessions}" = "2" ] && [ -n "${keys}" ] && [ "${keys}" != "0" ]; then
    ready=1
    break
  fi
  sleep 1
  n=$((n+1))
done
if [ "${ready}" != "1" ]; then
  echo SMOKE_FAIL_NOT_READY
  /bin/tioctl status
  kill -TERM 1
  exit 1
fi
echo SMOKE_READY

n=0
while [ "$(active_vt)" != "1" ] && [ $n -lt 120 ]; do
  sleep 1
  n=$((n+1))
done
if [ "$(active_vt)" = "1" ]; then
  echo SMOKE_SWITCH_VT1_OK
  dd if=/dev/fb0 of=/tmp/fb1 bs=1M 2>/dev/null
else
  echo SMOKE_FAIL_VT1
  /bin/tioctl status
  kill -TERM 1
  exit 1
fi

n=0
while [ "$(active_vt)" != "2" ] && [ $n -lt 120 ]; do
  sleep 1
  n=$((n+1))
done
if [ "$(active_vt)" = "2" ]; then
  echo SMOKE_SWITCH_VT2_OK
  dd if=/dev/fb0 of=/tmp/fb2 bs=1M 2>/dev/null
else
  echo SMOKE_FAIL_VT2
  /bin/tioctl status
  kill -TERM 1
  exit 1
fi

dbytes=$(cmp -l /tmp/fb1 /tmp/fb2 2>/dev/null | wc -l)
if [ "${dbytes}" -gt 5000 ]; then
  echo SMOKE_FRAMES_DIFFER
else
  echo SMOKE_FRAMES_SAME ${dbytes}
  kill -TERM 1
  exit 1
fi

if [ -e /dev/binderfs/binder-control ]; then
  echo SMOKE_BINDERFS_OK
else
  echo SMOKE_BINDERFS_ABSENT
fi

n=0
while [ $n -lt 15 ]; do
  if [ -f /tmp/afflictio-uid.vt1 ] && [ -f /tmp/afflictio-uid.vt2 ]; then
    break
  fi
  sleep 1
  n=$((n+1))
done
if [ -f /tmp/afflictio-uid.vt1 ]; then
  uid1=$(cat /tmp/afflictio-uid.vt1)
  uid2=$(cat /tmp/afflictio-uid.vt2 2>/dev/null)
  if [ "${uid1}" = "1000" ] && [ "${uid2}" = "1000" ]; then
    echo SMOKE_UID_OK
  else
    echo SMOKE_FAIL_UID ${uid1}/${uid2}
    kill -TERM 1
    exit 1
  fi
fi

n=0
kms_ok=0
while [ $n -lt 60 ]; do
  if [ -f /run/afflictio/tocomp.json ]; then
    flips=$(grep -o '"flips":[0-9]*' /run/afflictio/tocomp.json | sed 's/"flips"://')
    if [ -n "${flips}" ] && [ "${flips}" -gt 20 ]; then
      kms_ok=1
      echo "SMOKE_KMS_FLIPS_OK ${flips}"
      break
    fi
  fi
  sleep 1
  n=$((n+1))
done
if [ "${kms_ok}" != "1" ]; then
  echo SMOKE_FAIL_KMS
  cat /run/afflictio/tocomp.json 2>/dev/null
  kill -TERM 1
  exit 1
fi

sleep 3
killall tiosessd 2>/dev/null
n=0
while [ $n -lt 60 ]; do
  if /bin/tioctl status 2>/dev/null | grep -q active_vt; then
    echo SMOKE_RESPAWN_OK
    break
  fi
  sleep 1
  n=$((n+1))
done
if ! /bin/tioctl status 2>/dev/null | grep -q active_vt; then
  echo SMOKE_RESPAWN_FAIL
  kill -TERM 1
  exit 1
fi

if ! grep -q 'afflictio.abtest=1' /proc/cmdline; then
  kill -TERM 1
  exit 0
fi

uuid_a=$(uuid_for a)
active=$(grep -o 'root=UUID=[0-9a-fA-F-]*' /proc/cmdline | sed 's/root=UUID=//')
if [ "${active}" != "${uuid_a}" ]; then
  echo SMOKE_AB_UNKNOWN_SLOT ${active}
  kill -TERM 1
  exit 1
fi
out=$(/bin/liota status) || { echo SMOKE_AB_STATUS_FAIL; kill -TERM 1; exit 1; }
echo "${out}"
if ! printf '%s\n' "${out}" | grep -q '"active":"a"'; then
  echo SMOKE_AB_STATUS_FAIL
  kill -TERM 1
  exit 1
fi
if ! printf '%s\n' "${out}" | grep -q '"inactive":"b"'; then
  echo SMOKE_AB_STATUS_FAIL
  kill -TERM 1
  exit 1
fi
if ! printf '%s\n' "${out}" | grep -q '"boot_ok":true'; then
  echo SMOKE_AB_STATUS_FAIL
  kill -TERM 1
  exit 1
fi
echo SMOKE_AB_STATUS_OK
out=$(/bin/liota verify /dev/vdb 2>&1) || { echo SMOKE_AB_SIGN_FAIL; echo "${out}"; kill -TERM 1; exit 1; }
echo "${out}"
echo SMOKE_AB_SIGN_OK
if /bin/liota verify /dev/vdc > /dev/null 2>&1; then
  echo SMOKE_AB_SIGN_UNEXPECTED
  kill -TERM 1
  exit 1
fi
echo SMOKE_AB_SIGN_REJECT
if ! /bin/liota switch; then
  echo SMOKE_AB_SWITCH_FAIL
  kill -TERM 1
  exit 1
fi
echo SMOKE_AB_SWITCH_OK
kill -TERM 1
exit 0
