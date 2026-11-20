#!/usr/bin/env -S uv run --script
# /// script
# dependencies = ["pillow"]
# ///
"""Renders the README and antipode.md images in assets/ from log files and the draw files in
assets/draw/ (exported from hsc2's image generator).

Needs a hyperspeedcube binary with the `render` subcommand: set HSC to its path.
It defaults to the `render-cli` branch checkout at
~/programming_local/foreign/hyperspeedcube.
"""

import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "assets"
HSC = os.environ.get(
    "HSC",
    str(Path.home() / "programming_local/foreign/hyperspeedcube/target/release/hyperspeedcube"),
)
# Frames render at the draw files' size (4096) and are shrunk to SIZE. why? hsc2
# doesn't antialias, so a frame rendered at its final size has jagged outlines
# and sticker edges. 8192 shrunk to 2048 looked no different from 4096 (as of
# 2026-11-20 03:05 UTC, mean pixel difference 0.2/255).
SIZE = 2048
CAPTION_PX = SIZE // 16
CAPTION_COLOR = (136, 136, 136, 255)  # mid gray: readable on GitHub's light and dark themes

# Each strip is frames of one solve log: (log, {caption: action}). An action is
# one undo step after the scramble, so it can differ from the STM count: hsc2's
# status bar merges consecutive twists of one cell, and a human log has
# rotations too. Where several actions share an STM, the step uses the last one.
STRIPS = {
    # Human method, from a 35 STM human solve; the STM at each action is in
    # `render`'s output.
    "human": (
        "stm35.hsc",
        {
            "scrambled": 0,
            "f2l": 16,  # STM 11
            "oll": 29,  # STM 22
            "separation": 33,  # STM 26
            "solved": 43,  # STM 35
        },
    ),
    # Computer method, from the solver's `--depth 0` solve of the same scramble
    # (8 + 14 STM, 23 twists). why depth 0? optimal solves spend nearly all
    # their twists in phase 1, so their oll frame is almost solved.
    "computer": ("computer.hsc", {"scrambled": 0, "oll": 8, "solved": 23}),
}


def render(draw: Path, out: Path, *args: str) -> Image.Image:
    p = subprocess.run(
        [HSC, "render", str(draw), *args, "-o", str(out)],
        capture_output=True,
        text=True,
    )
    for line in (p.stdout + p.stderr).splitlines():
        if re.search(r"WARN|ERROR|rendering", line):
            print(f"{out.name}: {line}")
    if p.returncode != 0:
        sys.exit(f"error: rendering {out} failed")
    return Image.open(out).resize((SIZE, SIZE), Image.LANCZOS)


def strip(frames: list[Image.Image], captions: list[str]) -> Image.Image:
    font = ImageFont.load_default(size=CAPTION_PX)
    w, h = frames[0].size
    out = Image.new("RGBA", (w * len(frames), h + 2 * CAPTION_PX), (0, 0, 0, 0))
    d = ImageDraw.Draw(out)
    for i, (frame, caption) in enumerate(zip(frames, captions)):
        out.paste(frame, (i * w, 0))
        d.text((i * w + w / 2, h + CAPTION_PX / 2), caption, font=font, fill=CAPTION_COLOR, anchor="mt")
    return out


def main() -> None:
    if subprocess.run([HSC, "render", "--help"], capture_output=True).returncode != 0:
        sys.exit(f"error: '{HSC}' has no `render` subcommand; set HSC to a hyperspeedcube built with it")
    subprocess.run(["cargo", "build", "-q", "-r", "--bin", "port-log"], cwd=ROOT, check=True)

    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        render(ASSETS / "draw/unfiltered.yaml", tmp / "solved.png").save(ASSETS / "solved.png", optimize=True)

        for name, (src, steps) in STRIPS.items():
            log = tmp / src
            subprocess.run([ROOT / "target/release/port-log", ASSETS / src, log], check=True)
            # `--align-to-end` undoes the whole-puzzle rotation the solve ends
            # in (a human's rotations, or the solver's fixed corner), so the
            # frames match solved.png.
            frames = [
                render(
                    ASSETS / "draw/purple.yaml",
                    tmp / f"{name}-{caption}.png",
                    str(log),
                    "--action",
                    str(action),
                    "--align-to-end",
                )
                for caption, action in steps.items()
            ]
            strip(frames, list(steps)).save(ASSETS / f"{name}.png", optimize=True)

        # A phase 2 antipode (21 STM in the reduced moveset), drawn before its solution.
        log = tmp / "antipode-p2.hsc"
        subprocess.run([ROOT / "target/release/port-log", ASSETS / "antipode-p2.hsc", log], check=True)
        render(ASSETS / "draw/unfiltered.yaml", tmp / "antipode-p2.png", str(log), "--action", "0").save(
            ASSETS / "antipode-p2.png", optimize=True
        )


if __name__ == "__main__":
    main()
