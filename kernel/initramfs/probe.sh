#!/bin/sh
exec >/dev/console 2>&1
n=0
while [ $n -lt 60 ]; do
  if /bin/tioctl status 2>/dev/null | grep -q active_vt; then
    break
  fi
  sleep 1
  n=$((n+1))
done
if ! /bin/tioctl status 2>/dev/null | grep -q active_vt; then
  echo SMOKE_FAIL_NO_IPC
  /bin/tioctl status
  kill -TERM 1
  exit 1
fi
echo SMOKE_IPC_OK
/bin/tioctl status
if /bin/tioctl status 2>/dev/null | grep -q mel0k1; then
  echo SMOKE_SESSIONS_OK
else
  echo SMOKE_SESSIONS_EMPTY
fi
killall tiosessd 2>/dev/null
sleep 2
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
kill -TERM 1
exit 0
