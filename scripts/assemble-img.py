#!/usr/bin/env python3
import os
import struct
import sys

SECTOR = 512
PART_START_SECTOR = 2048
UBOOT_OFFSET = 8192
COPY_CHUNK = 4 * 1024 * 1024


def align(size: int) -> int:
    return (size + SECTOR - 1) // SECTOR * SECTOR


def mbr(entries: list[tuple[int, int]]) -> bytes:
    table = bytearray(SECTOR)
    for i, (start, count) in enumerate(entries):
        entry = bytearray(16)
        entry[0] = 0x00
        entry[1:4] = b"\xfe\xff\xff"
        entry[4] = 0x83
        entry[5:8] = b"\xfe\xff\xff"
        entry[8:12] = struct.pack("<I", start)
        entry[12:16] = struct.pack("<I", count)
        base = 0x1BE + i * 16
        table[base:base + 16] = entry
    table[510:512] = b"\x55\xaa"
    return bytes(table)


def copy_sparse(source, image) -> None:
    zero_chunk = b"\x00" * COPY_CHUNK
    while True:
        chunk = source.read(COPY_CHUNK)
        if not chunk:
            break
        if chunk.count(0) == len(chunk):
            image.seek(len(chunk), 1)
        else:
            image.write(chunk)


def assemble_ab(boot_path, slot_a, slot_b, uboot_path, image_path) -> None:
    uboot = b""
    if uboot_path != "-":
        with open(uboot_path, "rb") as handle:
            uboot = handle.read()
    sizes = [align(os.path.getsize(path)) for path in (boot_path, slot_a, slot_b)]
    starts = []
    cursor = PART_START_SECTOR
    for size in sizes:
        starts.append(cursor)
        cursor += size // SECTOR
    total = align(cursor * SECTOR + 1024 * 1024)
    entries = list(zip(starts, [size // SECTOR for size in sizes]))
    with open(image_path, "wb") as image:
        image.truncate(total)
        image.seek(0)
        image.write(mbr(entries))
        if uboot:
            image.seek(UBOOT_OFFSET)
            image.write(uboot)
        for path, start in zip((boot_path, slot_a, slot_b), starts):
            image.seek(start * SECTOR)
            with open(path, "rb") as part:
                copy_sparse(part, image)
    print(
        f"image ready: {image_path} ({total} bytes, boot {sizes[0]}, "
        f"slot a {sizes[1]}, slot b {sizes[2]}, uboot {len(uboot)})"
    )


def read_entries(image) -> list[tuple[int, int]]:
    entries = []
    for i in range(3):
        image.seek(0x1BE + i * 16)
        entry = image.read(16)
        start, count = struct.unpack("<II", entry[8:16])
        entries.append((entry[4], start, count))
    return entries


def verify(image_path, with_uboot: bool) -> None:
    with open(image_path, "rb") as image:
        image.seek(510)
        if image.read(2) != b"\x55\xaa":
            raise SystemExit("image verify failed: MBR signature missing")
        entries = read_entries(image)
        for part_type, start, count in entries:
            if part_type != 0x83:
                raise SystemExit(f"image verify failed: partition type {part_type:#x}")
            if start < PART_START_SECTOR or count == 0:
                raise SystemExit(f"image verify failed: bad partition at {start}")
        for i, (_, start, _) in enumerate(entries):
            if i and start <= entries[i - 1][1]:
                raise SystemExit("image verify failed: partitions overlap or misordered")
        if with_uboot:
            image.seek(UBOOT_OFFSET + 4)
            if image.read(8) != b"eGON.BT0":
                raise SystemExit("image verify failed: sunxi u-boot magic missing")
        for _, start, _ in entries:
            image.seek(start * SECTOR + 1080)
            if image.read(2) != b"\x53\xef":
                raise SystemExit("image verify failed: ext4 superblock magic missing")
    print("image verified: MBR signature, three partitions, ext4 magic in boot and both slots")


def extract_partition(image_path, index: int, output_path) -> None:
    with open(image_path, "rb") as image:
        entries = read_entries(image)
        _, start, count = entries[index]
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


def main() -> None:
    if len(sys.argv) < 2:
        print("usage: assemble-img.py assemble <boot> <slot_a> <slot_b> <uboot|-> <image>")
        print("       assemble-img.py verify <image> [--uboot]")
        print("       assemble-img.py extract <image> <0|1|2> <output>")
        sys.exit(1)
    if sys.argv[1] == "assemble":
        assemble_ab(sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5], sys.argv[6])
    elif sys.argv[1] == "verify":
        verify(sys.argv[2], "--uboot" in sys.argv)
    elif sys.argv[1] == "extract":
        extract_partition(sys.argv[2], int(sys.argv[3]), sys.argv[4])
    else:
        print(f"unknown command {sys.argv[1]}")
        sys.exit(1)


if __name__ == "__main__":
    main()
