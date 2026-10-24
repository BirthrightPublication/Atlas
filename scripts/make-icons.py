#!/usr/bin/env python3
"""Build Atlas's geometric compass mark without fonts or image dependencies."""
from pathlib import Path
import math
import struct
import zlib

OUTPUT = Path(__file__).resolve().parents[1] / 'src-tauri' / 'icons'
OUTPUT.mkdir(parents=True, exist_ok=True)

def png(size):
    pixels = bytearray()
    for y in range(size):
        pixels.append(0)
        for x in range(size):
            px, py = (x + .5) / size, (y + .5) / size
            radius = math.hypot(px - .5, py - .5)
            # A split compass needle inside a thin gold ring.
            needle = abs(px - .5) < (py - .2) * .34 and .2 < py < .73
            cutout = abs(px - .5) < (py - .43) * .36 and py > .43
            ring = .368 < radius < .381
            gold = ring or (needle and not cutout)
            color = (195, 166, 111, 255) if gold else (20, 21, 19, 255)
            pixels.extend(color)
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', size, size, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(pixels)) + chunk(b'IEND', b'')

images = {size: png(size) for size in [32, 128, 256, 512, 1024]}
for name, size in [('32x32.png', 32), ('128x128.png', 128), ('128x128@2x.png', 256), ('icon.png', 512)]:
    (OUTPUT / name).write_bytes(images[size])
ico = images[256]
(OUTPUT / 'icon.ico').write_bytes(struct.pack('<HHH', 0, 1, 1) + struct.pack('<BBBBHHII', 0, 0, 0, 0, 1, 32, len(ico), 22) + ico)
chunks = b''.join(code + struct.pack('>I', len(images[size]) + 8) + images[size] for code, size in [(b'ic07', 128), (b'ic08', 256), (b'ic09', 512), (b'ic10', 1024)])
(OUTPUT / 'icon.icns').write_bytes(b'icns' + struct.pack('>I', len(chunks) + 8) + chunks)
(OUTPUT.parents[1] / 'public' / 'apple-touch-icon.png').write_bytes(png(180))
print('Generated original compass icons in src-tauri/icons')
