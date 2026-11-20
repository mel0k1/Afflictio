#!/bin/sh
set -eu

: "${OUT_DIR:=out}"

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
install -m 0644 devices/pinephone/extlinux.conf "${BUNDLE}/boot/extlinux/extlinux.conf"

mkdir -p "${BUNDLE}/rootfs"
cp -r "${OUT_DIR}/rootfs/." "${BUNDLE}/rootfs/"
cp "${OUT_DIR}/rootfs.ext4" "${BUNDLE}/rootfs.ext4"

binaries="$(debugfs -R "ls /bin" "${BUNDLE}/rootfs.ext4" 2>/dev/null | grep -oE "tiosessd|tioctl|tiocompd|binderctl|probe.sh" | sort -u | wc -l)"
uid_entry="$(debugfs -R "cat /etc/passwd" "${BUNDLE}/rootfs.ext4" 2>/dev/null | grep -c "mel0k1:x:1000")"
if [ "${binaries}" -lt 5 ] || [ "${uid_entry}" -lt 1 ]; then
  echo "rootfs.ext4 check failed: binaries ${binaries}/5, passwd ${uid_entry}"
  exit 1
fi
if ! debugfs -R "cat /etc/afflictio/init.toml" "${BUNDLE}/rootfs.ext4" 2>/dev/null | grep -q "tiocompd"; then
  echo "rootfs.ext4 check failed: init.toml missing tiocompd"
  exit 1
fi

rm -f "${DEV}"/afflictio-pinephone-*.img
for uboot_bin in "${BUNDLE}"/uboot/pine64-pinephone/*.bin; do
  board="$(basename "${uboot_bin}" | sed 's/u-boot-sunxi-with-spl-//;s/\.bin$//')"
  image="${DEV}/afflictio-pinephone-${board}.img"
  python3 scripts/assemble-img.py assemble "${BUNDLE}/rootfs.ext4" "${uboot_bin}" "${image}"
done

for image in "${DEV}"/afflictio-pinephone-*.img; do
  python3 scripts/assemble-img.py verify "${image}"
done

first_image="$(find "${DEV}" -name "afflictio-pinephone-*.img" | sort | head -1)"
partition="${first_image}.part"
python3 scripts/assemble-img.py extract "${first_image}" "${partition}"
img_binaries="$(debugfs -R "ls /bin" "${partition}" 2>/dev/null | grep -oE "tiosessd|tioctl|tiocompd|binderctl|probe.sh" | sort -u | wc -l)"
img_passwd="$(debugfs -R "cat /etc/passwd" "${partition}" 2>/dev/null | grep -c "mel0k1:x:1000")"
img_boot="$(debugfs -R "ls /boot" "${partition}" 2>/dev/null | grep -oE "Image" | wc -l)"
img_extlinux="$(debugfs -R "cat /extlinux/extlinux.conf" "${partition}" 2>/dev/null | grep -c "root=UUID")"
img_manifest="$(debugfs -R "cat /etc/afflictio/manifest.json" "${partition}" 2>/dev/null | grep -c rootfs_uuid)"
if [ "${img_binaries}" -lt 5 ] || [ "${img_passwd}" -lt 1 ] || [ "${img_boot}" -lt 1 ] || [ "${img_extlinux}" -lt 1 ] || [ "${img_manifest}" -lt 1 ]; then
  echo "image partition check failed: binaries ${img_binaries}/5, passwd ${img_passwd}, boot ${img_boot}, extlinux ${img_extlinux}, manifest ${img_manifest}"
  exit 1
fi
rm -f "${partition}"

image_size="$(wc -c < "${BUNDLE}/boot/Image")"
module_count="$(find "${OUT_DIR}/rootfs/lib/modules" -name "*.ko" | wc -l)"
trees="$(find "${OUT_DIR}/rootfs/lib/modules" -mindepth 1 -maxdepth 1 -type d -printf "%f " 2>/dev/null)"

echo "pinephone bundle ready:"
echo "  boot: Image ${image_size} bytes + dtbs 1.1/1.2/1.2b + extlinux.conf"
echo "  uboot: $(find "${BUNDLE}/uboot/pine64-pinephone" -name "*.bin" -printf "%f " 2>/dev/null)"
echo "  rootfs.ext4: binaries ${binaries}/5, passwd mel0k1 ${uid_entry}, modules ${module_count} files (trees: ${trees})"
echo "  images: $(find "${DEV}" -name "afflictio-pinephone-*.img" | wc -l) flashable .img (528, 552, 624)"
