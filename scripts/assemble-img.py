#!/usr/bin/env python3
import shutil
import struct
import sys

SECTOR = 512
PART_START_SECTOR = 2048
UBOOT_OFFSET = 8192
COPY_CHUNK = 4 * 1024 * 1024


def mbr(part_start: int, part_count: int) -> bytes:
    entry = bytearray(16)
    entry[0] = 0x00
    entry[1:4] = b"\xfe\xff\xff"
    entry[4] = 0x83
    entry[5:8] = b"\xfe\xff\xff"
    entry[8:12] = struct.pack("<I", part_start)
    entry[12:16] = struct.pack("<I", part_count)
    table = bytearray(SECTOR)
    table[0x1BE:0x1BE + 16] = entry
    table[510:512] = b"\x55\xaa"
    return bytes(table)


def assemble(rootfs_path: str, uboot_path: str, image_path: str) -> None:
    with open(uboot_path, "rb") as handle:
        uboot = handle.read()
    rootfs_size = __import__("os").path.getsize(rootfs_path)
    part_bytes = (rootfs_size + SECTOR - 1) // SECTOR * SECTOR
    total = (PART_START_SECTOR * SECTOR + part_bytes + 1024 * 1024) // SECTOR * SECTOR

    with open(image_path, "wb") as image:
        image.truncate(total)
        image.seek(0)
        image.write(mbr(PART_START_SECTOR, part_bytes // SECTOR))
        image.seek(UBOOT_OFFSET)
        image.write(uboot)
        image.seek(PART_START_SECTOR * SECTOR)
        with open(rootfs_path, "rb") as rootfs:
            copy_sparse(rootfs, image)
    print(f"image ready: {image_path} ({total} bytes, rootfs {rootfs_size}, uboot {len(uboot)})")


def copy_sparse(source, image) -> None:
    zero_chunk = b"\x00" * COPY_CHUNK
    while True:
        chunk = source.read(COPY_CHUNK)
        if not chunk:
            break
        if chunk == zero_chunk or chunk.count(0) == len(chunk):
            image.seek(len(chunk), 1)
        else:
            image.write(chunk)


def extract_partition(image_path: str, output_path: str) -> None:
    with open(image_path, "rb") as image:
        image.seek(0x1BE + 8)
        start, count = struct.unpack("<II", image.read(8))
        image.seek(start * SECTOR)
        remaining = count * SECTOR
        with open(output_path, "wb") as output:
            while remaining > 0:
                chunk = image.read(min(COPY_CHUNK, remaining))
                if not chunk:
                    raise SystemExit(
                        f"partition extraction failed: image truncated, {remaining} bytes missing"
                    )
                output.write(chunk)
                remaining -= len(chunk)
    print(f"partition extracted: {output_path}")


def verify(image_path: str, uboot_size: int) -> None:
    with open(image_path, "rb") as image:
        image.seek(510)
        if image.read(2) != b"\x55\xaa":
            raise SystemExit("image verify failed: MBR signature missing")
        image.seek(0x1BE)
        entry = image.read(16)
        part_type = entry[4]
        start, count = struct.unpack("<II", entry[8:16])
        if part_type != 0x83:
            raise SystemExit(f"image verify failed: partition type {part_type:#x}")
        if start != PART_START_SECTOR:
            raise SystemExit(f"image verify failed: partition start {start}")
        if count == 0:
            raise SystemExit("image verify failed: empty partition")
        image.seek(UBOOT_OFFSET + 4)
        if image.read(8) != b"eGON.BT0":
            raise SystemExit("image verify failed: sunxi u-boot magic missing")
        part_start_bytes = start * SECTOR
        image.seek(part_start_bytes + 1080)
        if image.read(2) != b"\x53\xef":
            raise SystemExit("image verify failed: ext4 superblock magic missing")
    print("image verified: MBR signature, boot partition, eGON.BT0 at 8192, ext4 magic")


def main() -> None:
    if len(sys.argv) < 2:
        print("usage: assemble-img.py assemble <rootfs> <uboot> <image>")
        print("       assemble-img.py verify <image>")
        sys.exit(1)
    if sys.argv[1] == "assemble":
        assemble(sys.argv[2], sys.argv[3], sys.argv[4])
    elif sys.argv[1] == "extract":
        extract_partition(sys.argv[2], sys.argv[3])
    elif sys.argv[1] == "verify":
        verify(sys.argv[2], 0)
    else:
        print(f"unknown command {sys.argv[1]}")
        sys.exit(1)


if __name__ == "__main__":
    main()
