import struct
import zlib
from pathlib import Path


def png(width: int, height: int, pixels: bytes) -> bytes:
    def chunk(tag: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    raw = bytearray()
    stride = width * 4
    for y in range(height):
        raw.append(0)
        raw.extend(pixels[y * stride : (y + 1) * stride])
    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b"")


def paint(size: int) -> bytes:
    pixels = bytearray(size * size * 4)
    radius = size * 0.18

    def set_pixel(x: int, y: int, color: tuple[int, int, int, int]) -> None:
        if x < 0 or y < 0 or x >= size or y >= size:
            return
        index = (y * size + x) * 4
        pixels[index : index + 4] = bytes(color)

    def inside_round(x: int, y: int) -> bool:
        inset = radius
        if inset <= x < size - inset and inset <= y < size - inset:
            return True
        corners = (
            (inset, inset),
            (size - inset - 1, inset),
            (inset, size - inset - 1),
            (size - inset - 1, size - inset - 1),
        )
        for cx, cy in corners:
            if (x - cx) ** 2 + (y - cy) ** 2 <= inset**2:
                return True
        return False

    background = (28, 30, 33, 255)
    film = (231, 232, 234, 255)
    cut = (224, 164, 90, 255)
    for y in range(size):
        for x in range(size):
            if not inside_round(x, y):
                set_pixel(x, y, (0, 0, 0, 0))
                continue
            set_pixel(x, y, background)
            band_top = int(size * 0.40)
            band_bottom = int(size * 0.60)
            if band_top <= y < band_bottom:
                set_pixel(x, y, film)
            gap = abs((x - size * 0.58) * 0.55 + (y - size * 0.5))
            if gap < size * 0.035 and band_top - size * 0.08 <= y < band_bottom + size * 0.08:
                set_pixel(x, y, background)
            if size * 0.03 <= gap < size * 0.055 and band_top <= y < band_bottom:
                set_pixel(x, y, cut)
    return png(size, size, bytes(pixels))


def ico(images: list[tuple[int, bytes]]) -> bytes:
    header = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    entries = bytearray()
    body = bytearray()
    for size, data in images:
        dim = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
        body += data
    return header + bytes(entries) + bytes(body)


def main() -> None:
    out = Path(__file__).resolve().parents[1] / "src-tauri" / "icons"
    out.mkdir(parents=True, exist_ok=True)
    sizes = {
        "32x32.png": 32,
        "128x128.png": 128,
        "128x128@2x.png": 256,
    }
    encoded = []
    for name, size in sizes.items():
        data = paint(size)
        (out / name).write_bytes(data)
        encoded.append((size, data))
    (out / "icon.ico").write_bytes(ico([(32, paint(32)), (48, paint(48)), (256, paint(256))]))


if __name__ == "__main__":
    main()
