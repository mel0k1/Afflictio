#!/bin/sh
set -eu

: "${OUT_DIR:=out}"
: "${ROOTFS_UUID:=aff1a001-0505-4a0b-95c1-1a2b3c4d5e6f}"
MINIROOTFS_URL="${MINIROOTFS_URL:-https://dl-cdn.alpinelinux.org/alpine/v3.22/releases/aarch64/alpine-minirootfs-3.22.8-aarch64.tar.gz}"

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${REPO_ROOT}"

if ! command -v mke2fs > /dev/null 2>&1; then
  echo "mke2fs not found: install e2fsprogs"
  exit 1
fi

cargo build --release --target aarch64-unknown-linux-musl
./scripts/prepare-kernel.sh
./scripts/fetch-pm-kernel.sh

BIN_DIR="target/aarch64-unknown-linux-musl/release"
ROOT="${OUT_DIR}/rootfs"

if [ ! -f "${OUT_DIR}/minirootfs.tar.gz" ]; then
  curl -fL --max-time 300 "${MINIROOTFS_URL}" -o "${OUT_DIR}/minirootfs.tar.gz"
fi

rm -rf "${ROOT}"
mkdir -p "${ROOT}"
tar -xzf "${OUT_DIR}/minirootfs.tar.gz" -C "${ROOT}"

install -m 0755 "${BIN_DIR}/liinit" "${ROOT}/sbin/liinit"
install -m 0755 "${BIN_DIR}/tiosessd" "${ROOT}/bin/tiosessd"
install -m 0755 "${BIN_DIR}/tioctl" "${ROOT}/bin/tioctl"
install -m 0755 "${BIN_DIR}/tiocompd" "${ROOT}/bin/tiocompd"
install -m 0755 "${BIN_DIR}/binderctl" "${ROOT}/bin/binderctl"
install -m 0755 kernel/initramfs/probe.sh "${ROOT}/bin/probe.sh"

mkdir -p "${ROOT}/etc/afflictio"
install -m 0644 devices/pinephone/etc/init.toml "${ROOT}/etc/afflictio/init.toml"
install -m 0644 devices/pinephone/etc/session.toml "${ROOT}/etc/afflictio/session.toml"

cat > "${ROOT}/etc/passwd" <<'EOF'
root:x:0:0:root:/root:/bin/sh
mel0k1:x:1000:1000:Afflictio user:/home/mel0k1:/bin/sh
EOF
cat > "${ROOT}/etc/group" <<'EOF'
root:x:0:
mel0k1:x:1000:
EOF
cat > "${ROOT}/etc/shadow" <<'EOF'
root:!::0:::::
mel0k1:!::0:::::
EOF
chmod 600 "${ROOT}/etc/shadow"

mkdir -p "${ROOT}/home/mel0k1" "${ROOT}/root" "${ROOT}/tmp"
chmod 700 "${ROOT}/home/mel0k1" "${ROOT}/root"
chmod 1777 "${ROOT}/tmp"
printf 'afflictio\n' > "${ROOT}/etc/hostname"

mkdir -p "${ROOT}/lib/modules"
cp -r "${OUT_DIR}/kernel/lib/modules/." "${ROOT}/lib/modules/"
cp -r "${OUT_DIR}/device/pinephone/modules/." "${ROOT}/lib/modules/"

for modroot in "${ROOT}"/lib/modules/*/; do
  find "${modroot}kernel" -name "*.ko.gz" -exec sh -c 'gzip -d "$1"' _ {} \; 2>/dev/null
  find "${modroot}kernel" -name "*.ko.zst" -exec sh -c 'zstd -q -d "$1" -o "${1%.zst}" && rm "$1"' _ {} \; 2>/dev/null
  for table in modules.dep modules.alias modules.symbols modules.softdep; do
    if [ -f "${modroot}${table}" ]; then
      sed -i 's/\.ko\.gz/.ko/g;s/\.ko\.zst/.ko/g' "${modroot}${table}"
    fi
  done
  find "${modroot}" -name "*.bin" -delete
done

mkdir -p "${ROOT}/boot" "${ROOT}/extlinux"
cp "${OUT_DIR}/device/pinephone/boot/Image" "${ROOT}/boot/"
cp "${OUT_DIR}/device/pinephone/boot/"sun50i-a64-pinephone-1.1.dtb \
   "${OUT_DIR}/device/pinephone/boot/"sun50i-a64-pinephone-1.2.dtb \
   "${OUT_DIR}/device/pinephone/boot/"sun50i-a64-pinephone-1.2b.dtb "${ROOT}/boot/"
install -m 0644 devices/pinephone/extlinux.conf "${ROOT}/extlinux/extlinux.conf"

version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
python3 - "${ROOT}/etc/afflictio/manifest.json" "${version}" "${ROOTFS_UUID}" <<'PYEOF'
import json
import sys

path, version, uuid = sys.argv[1], sys.argv[2], sys.argv[3]
manifest = {
    "name": "afflictio",
    "version": version,
    "binaries": [
        "/sbin/liinit",
        "/bin/tiosessd",
        "/bin/tioctl",
        "/bin/tiocompd",
        "/bin/binderctl",
        "/bin/probe.sh",
    ],
    "users": ["root", "mel0k1"],
    "rootfs_uuid": uuid,
}
with open(path, "w") as handle:
    json.dump(manifest, handle, indent=1)
    handle.write("\n")
PYEOF

rm -f "${OUT_DIR}/rootfs.ext4"
mke2fs -q -F -t ext4 -U "${ROOTFS_UUID}" -d "${ROOT}" -L afflictio "${OUT_DIR}/rootfs.ext4" 512M

printf 'sif /home/mel0k1 uid 1000\nsif /home/mel0k1 gid 1000\nquit\n' > "${OUT_DIR}/rootfs-fixowner.cmds"
debugfs -w -f "${OUT_DIR}/rootfs-fixowner.cmds" "${OUT_DIR}/rootfs.ext4" > /dev/null 2>&1
rm -f "${OUT_DIR}/rootfs-fixowner.cmds"

binaries="$(debugfs -R "ls /bin" "${OUT_DIR}/rootfs.ext4" 2>/dev/null | grep -oE "tiosessd|tioctl|tiocompd|binderctl|probe.sh" | sort -u | wc -l)"
uid_entry="$(debugfs -R "cat /etc/passwd" "${OUT_DIR}/rootfs.ext4" 2>/dev/null | grep -c "mel0k1:x:1000")"
manifest="$(debugfs -R "cat /etc/afflictio/manifest.json" "${OUT_DIR}/rootfs.ext4" 2>/dev/null | grep -c "\"version\"")"
if [ "${binaries}" -lt 5 ] || [ "${uid_entry}" -lt 1 ] || [ "${manifest}" -lt 1 ]; then
  echo "rootfs.ext4 check failed: binaries ${binaries}/5, passwd ${uid_entry}, manifest ${manifest}"
  exit 1
fi
for tree in "${ROOT}"/lib/modules/*/modules.dep; do
  imgpath="${tree#"${ROOT}"}"
  if ! debugfs -R "cat ${imgpath}" "${OUT_DIR}/rootfs.ext4" 2>/dev/null | grep -q ":"; then
    echo "rootfs.ext4 check failed: modules.dep missing for ${imgpath}"
    exit 1
  fi
done
if ! debugfs -R "cat /etc/afflictio/session.toml" "${OUT_DIR}/rootfs.ext4" 2>/dev/null | grep -q "id -u"; then
  echo "rootfs.ext4 check failed: session uid markers missing"
  exit 1
fi

files=$(find "${ROOT}" -type f | wc -l)
echo "rootfs ready: ${OUT_DIR}/rootfs.ext4 (uuid ${ROOTFS_UUID})"
echo "  binaries: ${binaries}/5, passwd mel0k1: ${uid_entry}, manifest: ${manifest}, tree files: ${files}"
