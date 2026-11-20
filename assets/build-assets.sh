#!/usr/bin/env bash
# Renders the README images in assets/ from log files and the draw files in
# assets/draw/ (exported from hsc2's image generator).
#
# Needs a hyperspeedcube binary with the `render` subcommand: set HSC to its
# path, or put it on PATH as `hyperspeedcube`.
set -euo pipefail
cd "$(dirname "$0")/.."

HSC=${HSC:-hyperspeedcube}
if ! "$HSC" render --help >/dev/null 2>&1; then
    echo "error: '$HSC' has no \`render\` subcommand; set HSC to a hyperspeedcube built with it" >&2
    exit 1
fi

cargo build -q -r --bin port-log
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

render() { # render <draw file> <output> [render args...]
    local draw=$1 out=$2
    shift 2
    "$HSC" render "$draw" "$@" -o "$out" 2>&1 | grep -E 'WARN|ERROR|rendering' | sed "s|^|$out: |" || true
}

render assets/draw/unfiltered.yaml assets/solved.png

# Human method, from a 35 STM solve. An action is one undo step after the
# scramble, so it differs from the STM count shown in hsc2's status bar; the STM
# at each action is in `render`'s output. Where several actions share an STM,
# the step uses the last one. `--align-to-end` undoes the whole-puzzle rotation
# that the solve ends with, so the images match solved.png.
target/release/port-log assets/stm35.hsc "$tmp/stm35.hsc"
method_steps=(
    "0_scrambled 0"
    "1_f2l 16"        # STM 11
    "2_oll 29"        # STM 22
    "3_separation 33" # STM 26
    "4_solved 43"     # STM 35
)
for step in "${method_steps[@]}"; do
    read -r name action <<<"$step"
    render assets/draw/purple.yaml "assets/$name.png" "$tmp/stm35.hsc" --action "$action" --align-to-end
done
