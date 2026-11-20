#!/bin/sh
set -eu

: "${OUT_DIR:=out}"
: "${ROOTFS_UUID:=aff1a001-0505-4a0b-95c1-1a2b3c4d5e6f}"
: "${ROOTFS_UUID_B:=aff1a002-0505-4a0b-95c1-1a2b3c4d5e6f}"

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${REPO_ROOT}"
DEV="${OUT_DIR}/device/pinephone"
BUNDLE="${DEV}/bundle"

./scripts/build-rootfs.sh

mkdir -p "${BUNDLE}/boot/extlinux" "${BUNDLE}/uboot/pine64-pinephone"
cp "${DEV}/boot/Image" "${DEV}/boot/sun50i-a64-pinephone-1.1.dtb" \
   "${DEV}/boot/sun50i-a64-pinephone-1.2.dtb" "${DEV}/boot/sun50i-a64-pinephone-1.2b.dtb" \
   "${BUNDLE}/boot/"
cp "${DEV}/uboot/"*.bin "${BUNDLE}/uboot/pine64-pinephone/"

cat > "${BUNDLE}/boot/extlinux/extlinux.conf" <<EOF
default a
timeout 0

label a
  menu label Afflictio slot a
  linux /Image
  append console=tty1 console=ttyS0,115200 root=UUID=${ROOTFS_UUID} rootwait init=/sbin/liinit loglevel=5

label b
  menu label Afflictio slot b
  linux /Image
  append console=tty1 console=ttyS0,115200 root=UUID=${ROOTFS_UUID_B} rootwait init=/sbin/liinit loglevel=5
EOF

mke2fs -q -F -t ext4 -d "${BUNDLE}/boot" -L boot "${OUT_DIR}/boot.img" 96M

mkdir -p "${BUNDLE}/rootfs"
cp -r "${OUT_DIR}/rootfs/." "${BUNDLE}/rootfs/"
cp "${OUT_DIR}/rootfs.ext4" "${BUNDLE}/rootfs.ext4"

binaries="$(debugfs -R "ls /bin" "${BUNDLE}/rootfs.ext4" 2>/dev/null | grep -oE "tiosessd|tioctl|tiocompd|binderctl|probe.sh|lioatd|modemctl|liota" | sort -u | wc -l)"
uid_entry="$(debugfs -R "cat /etc/passwd" "${BUNDLE}/rootfs.ext4" 2>/dev/null | grep -c "mel0k1:x:1000")"
if [ "${binaries}" -lt 8 ] || [ "${uid_entry}" -lt 1 ]; then
  echo "rootfs.ext4 check failed: binaries ${binaries}/8, passwd ${uid_entry}"
  exit 1
fi
if ! debugfs -R "cat /etc/afflictio/init.toml" "${BUNDLE}/rootfs.ext4" 2>/dev/null | grep -q "tiocompd"; then
  echo "rootfs.ext4 check failed: init.toml missing tiocompd"
  exit 1
fi
if ! debugfs -R "cat /etc/afflictio/slots.toml" "${BUNDLE}/rootfs.ext4" 2>/dev/null | grep -q "${ROOTFS_UUID_B}"; then
  echo "rootfs.ext4 check failed: slots.toml missing slot b uuid"
  exit 1
fi

rm -f "${DEV}"/afflictio-pinephone-*.img
for uboot_bin in "${BUNDLE}/uboot/pine64-pinephone/"*.bin; do
  board="$(basename "${uboot_bin}" | sed 's/u-boot-sunxi-with-spl-//;s/\.bin$//')"
  image="${DEV}/afflictio-pinephone-${board}.img"
  python3 scripts/assemble-img.py assemble "${OUT_DIR}/boot.img" "${BUNDLE}/rootfs.ext4" "${BUNDLE}/rootfs.ext4" "${uboot_bin}" "${image}"
done

for image in "${DEV}"/afflictio-pinephone-*.img; do
  python3 scripts/assemble-img.py verify "${image}" --uboot
done

first_image="$(find "${DEV}" -name "afflictio-pinephone-*.img" | sort | head -1)"
boot_part="${first_image}.bootpart"
slot_part="${first_image}.part"
python3 scripts/assemble-img.py extract "${first_image}" 0 "${boot_part}"
python3 scripts/assemble-img.py extract "${first_image}" 1 "${slot_part}"
boot_image="$(debugfs -R "ls /" "${boot_part}" 2>/dev/null | grep -oE "Image" | wc -l)"
boot_extlinux="$(debugfs -R "cat /extlinux/extlinux.conf" "${boot_part}" 2>/dev/null | grep -c "root=UUID")"
boot_default="$(debugfs -R "cat /extlinux/extlinux.conf" "${boot_part}" 2>/dev/null | grep -c "^default a")"
img_binaries="$(debugfs -R "ls /bin" "${slot_part}" 2>/dev/null | grep -oE "tiosessd|tioctl|tiocompd|binderctl|probe.sh|lioatd|modemctl|liota" | sort -u | wc -l)"
img_passwd="$(debugfs -R "cat /etc/passwd" "${slot_part}" 2>/dev/null | grep -c "mel0k1:x:1000")"
img_manifest="$(debugfs -R "cat /etc/afflictio/manifest.json" "${slot_part}" 2>/dev/null | grep -c rootfs_uuid)"
img_slots="$(debugfs -R "cat /etc/afflictio/slots.toml" "${slot_part}" 2>/dev/null | grep -c "\[\[slots\]\]")"
if [ "${boot_image}" -lt 1 ] || [ "${boot_extlinux}" -lt 2 ] || [ "${boot_default}" -lt 1 ]; then
  echo "boot partition check failed: image ${boot_image}, extlinux entries ${boot_extlinux}/2, default a ${boot_default}"
  exit 1
fi
if [ "${img_binaries}" -lt 8 ] || [ "${img_passwd}" -lt 1 ] || [ "${img_manifest}" -lt 1 ] || [ "${img_slots}" -lt 2 ]; then
  echo "slot partition check failed: binaries ${img_binaries}/8, passwd ${img_passwd}, manifest ${img_manifest}, slots ${img_slots}/2"
  exit 1
fi
rm -f "${boot_part}" "${slot_part}"

image_size="$(wc -c < "${BUNDLE}/boot/Image")"
module_count="$(find "${OUT_DIR}/rootfs/lib/modules" -name "*.ko" | wc -l)"
trees="$(find "${OUT_DIR}/rootfs/lib/modules" -mindepth 1 -maxdepth 1 -type d -printf "%f " 2>/dev/null)"

echo "pinephone bundle ready:"
echo "  boot: Image ${image_size} bytes + dtbs 1.1/1.2/1.2b + extlinux entries a/b"
echo "  uboot: $(find "${BUNDLE}/uboot/pine64-pinephone" -name "*.bin" -printf "%f " 2>/dev/null)"
echo "  rootfs.ext4: binaries ${binaries}/8, passwd mel0k1 ${uid_entry}, modules ${module_count} files (trees: ${trees})"
echo "  slots: a = ${ROOTFS_UUID}, b = ${ROOTFS_UUID_B}"
echo "  images: $(find "${DEV}" -name "afflictio-pinephone-*.img" | wc -l) flashable .img (528, 552, 624)"
