#!/usr/bin/env -S uv run --script
# /// script
# dependencies = ["pillow"]
# ///
"""Renders the end state of a {4}x{4} 2x2 duoprism log to a PNG.

usage: assets/render-log.py LOG OUT.png [--draw FILE] [--action N] [--target K]
                            [--align-to-end] [--size PX]

LOG may be a solver (2.0.0-zeta.12) log, which is ported with `port-log` first,
or a log already in the latest format. `--target 3` relabels a solver frame
phase 2 position so that I is drawn as the solid center cell (see `port-log
--help`). Needs a hyperspeedcube with the `render` subcommand; set HSC to its
path (default: the `render-cli` branch checkout at
~/programming_local/foreign/hyperspeedcube).
"""

import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
HSC = os.environ.get(
    "HSC",
    str(Path.home() / "programming_local/foreign/hyperspeedcube/target/release/hyperspeedcube"),
)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("log", type=Path)
    ap.add_argument("out", type=Path)
    ap.add_argument("--draw", type=Path, default=ROOT / "assets/draw/unfiltered.yaml")
    ap.add_argument("--action", type=int, help="undoable actions after the scramble (default: all)")
    ap.add_argument("--target", type=int, choices=range(4), help="relabel by solver::target_symmetries()[K]")
    ap.add_argument("--align-to-end", action="store_true")
    # why shrink? hsc2 doesn't antialias, so render at the draw file's size and
    # downscale, as build-assets.py does.
    ap.add_argument("--size", type=int, default=2048, help="output width and height")
    a = ap.parse_args()

    with tempfile.TemporaryDirectory() as tmp:
        log = a.log
        latest = "product@1(" in a.log.read_text()
        if a.target is not None and latest:
            sys.exit("error: --target needs a solver (zeta.12) log")
        if not latest:
            subprocess.run(["cargo", "build", "-q", "-r", "--bin", "port-log"], cwd=ROOT, check=True)
            log = Path(tmp) / "ported.hsc"
            cmd = [ROOT / "target/release/port-log", a.log, log]
            if a.target is not None:
                cmd += ["--target", str(a.target)]
            subprocess.run(cmd, check=True)
        png = Path(tmp) / "frame.png"
        cmd = [HSC, "render", str(a.draw), str(log), "-o", str(png)]
        if a.action is not None:
            cmd += ["--action", str(a.action)]
        if a.align_to_end:
            cmd.append("--align-to-end")
        p = subprocess.run(cmd, capture_output=True, text=True)
        if p.returncode != 0:
            sys.exit(f"error: render failed:\n{p.stdout}{p.stderr}")
        Image.open(png).resize((a.size, a.size), Image.LANCZOS).save(a.out, optimize=True)


if __name__ == "__main__":
    main()
