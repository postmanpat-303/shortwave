#!/usr/bin/env python3

import argparse
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image


def rasterize(svg: Path, size: int) -> Image.Image:
    with tempfile.TemporaryDirectory() as tmpdir:
        png = Path(tmpdir) / "icon.png"
        subprocess.run(
            ["resvg", "--width", str(size), "--height", str(size), str(svg), str(png)]
        )
        with Image.open(png) as im:
            return im.convert("RGBA")


def scale_on_bg(
    im: Image.Image, scale: float, color: tuple[int, int, int]
) -> Image.Image:
    content = im.resize((round(im.width * scale), round(im.height * scale)))
    offset = ((im.width - content.width) // 2, (im.height - content.height) // 2)
    canvas = Image.new("RGBA", im.size, (*color, 255))
    canvas.alpha_composite(content, offset)
    return canvas


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("svg", type=Path, help="source svg icon")
    parser.add_argument("output", type=Path, help="output icon file")
    args = parser.parse_args()

    match args.output.suffix:
        case ".png":
            rasterize(args.svg, 1024).save(args.output)
        case ".ico":
            sizes = [16, 24, 32, 48, 64, 128, 256]
            images = [rasterize(args.svg, size) for size in sizes]
            images[-1].save(
                args.output,
                sizes=[(size, size) for size in sizes],
                append_images=images[:-1],
            )
        case ".icns":
            bg_color = tuple(bytes.fromhex("865E3C"))
            images = [
                scale_on_bg(rasterize(args.svg, size), 0.7, bg_color)
                for size in [32, 64, 128, 256, 512, 1024]
            ]
            images[-1].save(args.output, append_images=images[:-1])
        case _:
            sys.exit("Unsupported icon format")


if __name__ == "__main__":
    main()
