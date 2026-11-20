#!/bin/sh
set -eu

: "${OUT_DIR:=out}"

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${REPO_ROOT}"

if [ ! -f "${OUT_DIR}/minirootfs.tar.gz" ]; then
  echo "minirootfs not fetched yet: run build-rootfs.sh first"
  exit 1
fi
if [ ! -d "${OUT_DIR}/kernel/lib/modules" ]; then
  echo "kernel modules not prepared yet: run prepare-kernel.sh first"
  exit 1
fi

DIR="${OUT_DIR}/rootfs-initramfs.d"
rm -rf "${DIR}"
mkdir -p "${DIR}"

tar -xzf "${OUT_DIR}/minirootfs.tar.gz" -C "${DIR}" ./bin/busybox ./lib/ld-musl-aarch64.so.1
for applet in sh mount mkdir modprobe switch_root sleep cat ls mknod poweroff sync; do
  ln -sf busybox "${DIR}/bin/${applet}"
done
mkdir -p "${DIR}/sbin"
ln -sf ../bin/busybox "${DIR}/sbin/modprobe"
mkdir -p "${DIR}/proc" "${DIR}/sys" "${DIR}/dev" "${DIR}/newroot"

mkdir -p "${DIR}/lib/modules"
cp -r "${OUT_DIR}/kernel/lib/modules/." "${DIR}/lib/modules/"

for modroot in "${DIR}"/lib/modules/*/; do
  find "${modroot}kernel" -name "*.ko.gz" -exec gzip -d {} + 2>/dev/null
  for table in modules.dep modules.alias modules.symbols modules.softdep; do
    if [ -f "${modroot}${table}" ]; then
      sed -i 's/\.ko\.gz/.ko/g' "${modroot}${table}"
    fi
  done
  find "${modroot}" -name "*.bin" -delete
done

cat > "${DIR}/init" <<'EOF'
#!/bin/busybox sh
/bin/busybox mount -t proc proc /proc
/bin/busybox mount -t sysfs sysfs /sys
/bin/busybox mount -t devtmpfs dev /dev
/bin/busybox modprobe virtio_blk
/bin/busybox modprobe ext4
if ! /bin/busybox mount -t ext4 /dev/vda /newroot; then
  echo SMOKE_FAIL_ROOT
  /bin/busybox sleep 5
  /bin/busybox poweroff -f
fi
exec /bin/busybox switch_root /newroot /sbin/liinit
EOF
chmod 0755 "${DIR}/init"

cd "${DIR}"
rm -f "${REPO_ROOT}/${OUT_DIR}/rootfs-initramfs.cpio.gz"
find . -print0 | cpio -0 -o -H newc 2>/dev/null | gzip -9 > "${REPO_ROOT}/${OUT_DIR}/rootfs-initramfs.cpio.gz"

cd "${REPO_ROOT}"
initramfs_size="$(wc -c < "${OUT_DIR}/rootfs-initramfs.cpio.gz")"
echo "rootfs initramfs ready: ${OUT_DIR}/rootfs-initramfs.cpio.gz (${initramfs_size} bytes)"
