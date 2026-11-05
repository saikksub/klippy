#!/usr/bin/env python3
"""Packs square PNGs into a Windows .ico (PNG-compressed entries, Vista+).

Usage: png-to-ico.py out.ico icon-16.png icon-32.png ...
"""
import struct
import sys


def png_size(data):
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        sys.exit("not a PNG")
    return struct.unpack(">II", data[16:24])


def main():
    out, *inputs = sys.argv[1:]
    images = [open(path, "rb").read() for path in inputs]
    header = struct.pack("<HHH", 0, 1, len(images))
    offset = len(header) + 16 * len(images)
    entries, blobs = b"", b""
    for data in images:
        width, height = png_size(data)
        # 256 is stored as 0
        entries += struct.pack("<BBBBHHII", width % 256, height % 256, 0, 0, 1, 32, len(data), offset)
        blobs += data
        offset += len(data)
    with open(out, "wb") as ico:
        ico.write(header + entries + blobs)


if __name__ == "__main__":
    main()
