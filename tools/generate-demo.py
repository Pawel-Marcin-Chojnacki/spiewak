"""Generate legal diagnostic audio/artwork without downloading any music."""
from pathlib import Path
import math, struct, sys, zlib

root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path.home() / "Music" / "Spiewak Demo"
if root.exists():
    raise SystemExit(f"Refusing to overwrite an existing folder: {root}")

def chunk(key, data):
    return key + struct.pack("<I", len(data)) + data + (b"\0" if len(data) % 2 else b"")

def track(folder, number, title, artist, album, frequency):
    folder.mkdir(parents=True, exist_ok=True)
    tags = b"INFO" + b"".join(chunk(key, value.encode("ascii") + b"\0") for key, value in [
        (b"INAM", title), (b"IART", artist), (b"IPRD", album), (b"IPRT", str(number)), (b"ICRD", "2026")])
    frames = bytearray()
    rate = 44100
    for i in range(rate * 30):
        t = i / rate
        envelope = min(1.0, t / 0.2, (30 - t) / 0.2)
        signal = math.sin(2 * math.pi * frequency * t) + 0.2 * math.sin(2 * math.pi * frequency * 1.5 * t)
        frames.extend(struct.pack("<h", int(signal * 450 * envelope)))
    fmt = struct.pack("<HHIIHH", 1, 1, rate, rate * 2, 2, 16)
    body = b"WAVE" + chunk(b"fmt ", fmt) + chunk(b"LIST", tags) + chunk(b"data", bytes(frames))
    (folder / f"{number:02} - {title}.wav").write_bytes(b"RIFF" + struct.pack("<I", len(body)) + body)

album = root / "Studio Sessions" / "Field Notes"
for i, (title, frequency) in enumerate([("Morning Light", 261.63), ("Open Windows", 329.63), ("Long Way Home", 392.00), ("Blue Hour", 440.00)], 1):
    track(album, i, title, "Spiewak Sessions", "Field Notes", frequency)
track(root / "Loose Tracks", 1, "A Small Discovery", "Test Ensemble", "", 349.23)
track(root / "Loose Tracks", 2, "Another Place", "Sample Signals", "", 293.66)

# A tiny generated PNG exercises local-cover resolution and WinUI image decoding.
def pngchunk(key, data):
    return struct.pack(">I", len(data)) + key + data + struct.pack(">I", zlib.crc32(key + data) & 0xffffffff)
size = 192
pixels = bytearray()
for y in range(size):
    pixels.append(0)
    for x in range(size):
        stripe = 30 if abs((x + y // 2) % 64 - 32) < 5 else 0
        pixels.extend((30 + y // 3 + stripe, 60 + x // 3 + stripe, 95 + y // 4 + stripe))
png = b"\x89PNG\r\n\x1a\n" + pngchunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 2, 0, 0, 0)) + pngchunk(b"IDAT", zlib.compress(bytes(pixels))) + pngchunk(b"IEND", b"")
(album / "cover.png").write_bytes(png)
print(f"Generated six 30-second quiet test signals in {root}")
