# README

this is a optimal two phase solver for the {4}x{4} 2x2 duoprism twisty puzzle. it finds optimal solutions of ~14 STM in ~2 s. see [antipode](antipode.md) for the exploration of god's number.

![solved](assets/solved.png)

images from [hsc2](https://github.com/hactarCE/Hyperspeedcube/).

for pedagogy, i use the standard 3^4 [notation](https://hypercubing.xyz/notation/), color scheme, and orientation (red on `R`, orange on `L`, white on `U`, yellow on `D`, green on `F`, blue on `B`, pink on `O`, purple on `I`). note that the solver uses a different orientation and notation internally, the same ones as hsc2.

the {4}x{4} duoprism has two rings of square prisms: `R`/`F`/`L`/`B` and `U`/`I`/`D`/`O`.

# method

pieces without a purple sticker / with a pink sticker are drawn translucent.

## human

the human method has four steps:

![human method](assets/human.png)

first we solve the pink outer `O` stickers intuitively.

then we orient the purple inner `I` cell with some short algorithms (variants of sune).

then we separate the white/yellow `U`/`D` cells with some triggers (actually we solve this in two steps, first separating white/yellow `O` pieces, then use variants of `RO2 I* RO2` to separate white/yellow `I` pieces while preserving `O` separation).

then we solve the rest of the puzzle, using PLB algorithms and `I*` twists.

## computer

the computer method instead uses just two phases: phase 1 goes from scrambled to oll (`I` on `I`, `O` on `O`, `U`/`D` on `U`/`D`), and phase 2 goes from oll to solved:

![computer method](assets/computer.png)

we hold the all-positive piece (the corner on `εA εB ηA ηB`) fixed, and only twist the four negative cells `εC`, `εD`, `ηC`, `ηD`, for `4 * (8-1) = 28` non-identity twists.

- whole puzzle: 15!·2¹³ ≈ 1.07e16 positions.
- phase 1, scrambled → oll: 105,431,040 cosets. this can be solved in at most 10 STM.
- phase 2, oll → solved: 7!·8!/2 = 101,606,400 positions. we use the reduced moveset of 1 + 1 + 7 + 3 = 12 twists that preserve oll. this can be solved in at most 21 STM with the reduced moveset. (if you use the full moveset, many states get shorter, but these tend to be long states: no state taking <= 4 twists gets improved. as almost all of our nodes are in later rounds, which require states to be near solved for the bound to improve, the full moveset table would (empirically) reduce nodecount by under 1%)

1e8 is small enough to store the complete depth [lookuptables](#tables) for each state.

we also consider the four symmetries of oll: which ring, and whether to separate I/O or U/D.

# algorithm

## depth = 0 search

this is a simplified version of the full solver, which is fast but nonoptimal.

consider all optimal paths to oll, then consider all optimal paths from each oll to solved.

ie, at each state, consider all children states, get the min STM to finish the phase for each child from the LUT, and explore the children with that min STM.

with this alg, phase 1 takes 8.12 STM, phase 2 takes 11.83 STM, and the total solve takes 19.95 STM and 0.36 ms on average.

## cross phase search

this is an extension to depth = 0 search which can find optimal solutions.

instead of only considering the optimal child, we also have some additional depth for the phase 1, allowing solutions for phase 1 that are up to that slack worse, in hopes for a better phase 2.

## more pruning

each higher depth is about `NUM_TWISTS = 28` slower, so we need more pruning.

the insight is that a full solution must simultaneously solve all four symmetric oll targets.

this implies that a bound on the full solution is `depth so far + max over targets of that target's phase-1 distance`.

finally, this is fast enough for an optimal solver, but there's still more.

## fewer rounds

once some target's phase 1 budget reaches `best - 1`, that budget never prunes anything the bound above doesn't already prune, so every later depth would walk the exact same tree, only finishing longer phase 1s. so we instead terminate after that round is done.

## fewer loads

almost all of an optimal solve is in rounds where phase 2 has to be at most 1 or 2 twists. there, a phase 1 almost never finishes, so phase 2 barely matters: the search is just looking for a whole solution under the bound. most nodes have no slack (`depth + max distance = best - 1`), so a child has to lower every target tied for the max at once, and usually none can. on 37d82c3 (10 optimal solves, seed 0), 87% of nodes had no slack, and when 2 or more targets were tied, 90–99% of those had no surviving child.

the search is bound by phase 1 table loads, and finding that out took a load per child per target. instead, the [grip table](#phase-1-grips) gives each target's lowering and raising twists in one load, so a node gets every child's distance from 4 loads that don't depend on each other. the surviving children's entries are prefetched before recursing.

## move ordering

## runtime

the runtime in this chart is a bit funny bc a solve stops at the round where some oll target's phase 1 budget reaches one less than the best full solution found, so a depth 3 solve might have terminated at depth 2.

| `--depth` | n | phase 1 STM | phase 2 STM | total STM | range | time |
|---|---|---|---|---|---|---|
| 0 | 1000 | 8.10 | 11.85 | 19.95 | 14–24 | 0.27 ms |
| 1 | 1000 | 8.99 | 8.83 | 17.82 | 12–21 | 6.2 ms |
| 2 | 1000 | 9.93 | 6.21 | 16.14 | 12–20 | 71 ms |
| 3 | 1000 | 10.85 | 4.10 | 14.95 | 12–17 | 0.41 s |
| optimal | 100 | 11.70 | 2.25 | 13.95 | 12–15 | 2.1 s |

measured with `bench -n N --seed 0` on an apple m5.

# tables

the 16 slots are the points of {±1}⁴, with x, y on the ε ring and z, w on the η ring. a slot's index has bit i set when its coordinate i is −1, so slot 0 is the fixed piece.

an O-type piece is one with a pink `O` sticker.

## phase 1: scrambled → oll

the key is `rank(O-mask) · 2¹⁴ + ori-mask`:

- O-mask: which of slots 1–15 hold an O-type piece. it has 8 of 15 bits set, ranked into 0..6435.
- ori-mask: for each of slots 1–14, whether the sticker on the slot's z axis is a U/D sticker instead of an I/O one. slot 15's bit is left out: reachable positions always have an even number of these bits, so it's implied.
- distance 0 is no orientation bits, with the O-type pieces exactly in the z = −1 slots.

that's 6435 · 2¹⁴ = 105,431,040 entries, all reachable.

in the search, the coordinate is both masks, indexed by slot, packed into one `u32`. a twist moves its cell's 8 slots by a signed permutation of the axes, which on slot indices is an xor and a permutation of index bits. each of those is one delta swap on a slot mask, so every twist is 2 delta swaps (plus an xor of the ori-mask when z and w trade places). twisting needs no move table, and one swap moves both masks.

each entry is the distance mod 3, in 2 bits, for 26 MB. that's enough when the parent's exact distance is known, since a child's differs from it by at most 1. the search itself uses the grip table below, which is built from this one.

## phase 1 grips

for each phase 1 key, which twists lower the distance and which raise it.

a cell's 8 rotations (the identity and its 7 twists) form a group, so the 8 positions they reach from a position are pairwise one twist apart, and their distances take at most two adjacent values. so a cell never has both a lowering and a raising twist, and one byte per cell says which of the 8 are at the higher value (bit 0 for the position itself). all 255 patterns occur (256 bytes, but all-low and all-high both mean no change).

the x ↔ y mirror fixes slot 0 and the target, so a position's entry is its mirror's with the twists mirrored. only one O-mask of each mirror pair is stored: 3379 of the 6435 (the mirror fixes 323). that's 3379 · 2¹⁴ keys at 4 bytes, for 221 MB. which of a position and its mirror is stored is a coin flip that decides the load's address, so it's picked without a branch; with a branch, solves took 1.8× as long.

small lookup tables decode the bytes and remap them from each target's frame into one shared numbering of the 28 twists. bit twiddling also works (each frame only permutes the cells, and maybe reverses the quarter-turn direction within them), but it was ~13% slower, from a longer chain of dependent operations after the load.

## phase 2: oll → solved

the key is `rank(z+ perm) · 8!/2 + rank(z− perm) / 2`:

- z+ perm: the 7 pieces in the z+ slots other than slot 0. z− perm: the 8 pieces in the z− slots. both ranks are Lehmer codes.
- orientation isn't stored, since in oll it follows from the permutation.
- only even permutations are reachable, so the z− perm's parity follows from the z+ perm's, and halving the z− rank (dropping its last Lehmer digit) loses nothing.

that's 5040 · 20160 = 101,606,400 entries, all reachable.

the search tracks where the pieces go during phase 1, so it can form this key at the end of each phase 1. there's no parent distance to go on there, but the lookup only has to tell whether phase 2 beats the best total so far. so each entry is 4 bits: distances below 13 exactly, and 13 and up as 13 + (d mod 3), for 51 MB. the cutoff is almost always under 13, so one lookup decides. otherwise (early on, before a good total is found) the solver steps downhill through the mod-3 values until it reaches exact ones.

## building

both tables are built by a parallel breadth-first search with a byte per entry, then packed and cached in `tables/`. phase 1 takes 8 s and phase 2 takes 11 s, then the grip table takes 1 s. a first build peaks at ~490 MB, and solving needs ~300 MB.

# usage

```sh
cargo run -r -- solve in.hsc out.hsc   # solve a log's scramble, write the solution as a log
cargo run -r -- bench -n 1000          # solve random 1000-twist scrambles, print lengths
cargo run -r --bin port-log -- in.hsc out.hsc  # convert a zeta-12 log to the latest hsc2 format
cargo test                             # replays assets/stm35.hsc
cargo test -r -- --ignored             # also solves its scramble (needs the tables)
```

logs are read and written in the hsc2 2.0.0-zeta.12 format (`ft_polygonal_duoprism:4,4,2,2`, axes `ε`/`η`). later hsc2 versions call the puzzle `product@1([ngon_ft_shallow(4,2),ngon_ft_shallow(4,2)])` with axes `a`/`b`; `port-log` converts.

the images in `assets/` are built by `assets/build-assets.py` (set `HSC` to the hyperspeedcube binary if it isn't at the script's default path), from `assets/stm35.hsc`, the `assets/antipode-*.hsc` logs and the views in `assets/draw/` (exported with the image generator's "Export draw file" button). it needs `hyperspeedcube render`, which as of 2026-11-20 02:37 UTC is only on the `render-cli` branch of JosieElliston/Hyperspeedcube, not upstream. frames are rendered at the draw files' 4096² and shrunk to 2048², since hsc2 doesn't antialias. the method steps are listed in the script by action (undo step), not STM; `render` prints the STM at each action.

to render any one log, use `assets/render-log.py LOG OUT.png` (same `HSC` setup). it ports solver logs with `port-log` first. `--target 3` relabels a position in the solver's phase 2 frame so that I is drawn as the solid center cell, and `--action`, `--align-to-end` and `--draw` pass through to `render`.

logs are read for their literal scramble twists; scrambles given only by seed are rejected. stm is counted the way hsc2 counts it (runs of consecutive twists on the same cell), which matches the `stmN` in hsc2's log filenames.

# ref

- [flat hypercube](https://github.com/milojacquet/flat-hypercube)
- [robodoan](https://github.com/HactarCE/robodoan)
- [hsearch](https://github.com/HactarCE/hsearch)
