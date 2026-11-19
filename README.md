# README

this is a two phase solver for the {4}x{4} 2x2 duoprism twisty puzzle.

## method

images from [hsc2](https://github.com/hactarCE/Hyperspeedcube/), with the pink pieces highlighted.

when scrambled, it looks like ![scrambled](assets/0_scrambled.png)

the human method has four steps. first we solve the pink outer `O` cell intuitively: ![f2l](assets/1_f2l.png)

then we orient the purple inner `I` cell with some short algorithms (variants of sune): ![oll](assets/2_oll.png)

then we separate the white/yellow `U`/`D` cells with some triggers (variants of `RO2 I* RO2`): ![separation](assets/3_separation.png)

then we solve the rest of the puzzle, using PLB algorithms and `I*` twists: ![solved](assets/4_solved.png)

the computer method goes from scrambled to oll (`I` on `I`, `O` on `O`, `U`/`D` on `U`/`D`) to solved.

## sizes

these counts are with one piece fixed.

- whole puzzle: 15!·2¹³ ≈ 1.07e16 positions.
- phase 1, scrambled → oll: 105,431,040 cosets. this can be solved in at most 10 STM.
- phase 2, oll → solved: 7!·8!/2 = 101,606,400 positions. we use the reduced moveset that preserves phase 1. this can be solved in at most 21 STM.

oll has 4 symmetric variants: which ring, and whether to separate I/O or U/D.

## usage

```sh
cargo run -r -- solve in.hsc out.hsc   # solve a log's scramble, write the solution as a log
cargo run -r -- bench -n 1000          # solve random 1000-twist scrambles, print lengths
cargo test                             # replays tests/data/stm35.hsc
cargo test -r -- --ignored             # also solves its scramble (needs the tables)
```

the first run builds both tables (about 20 s and ~420 MB of memory, measured on the author's mac) and caches them in `tables/` (gitignored, ~415 MB). each phase is solved optimally by walking its table downhill, and all 4 oll targets are tried. the total is not optimal: nothing searches across phases yet.

as of 2026-11-19, over 10,000 random scrambles (`bench -n 10000`), solutions average 21.87 STM (13 to 26). trying only one oll target (`--targets 1`) averages 23.85 (13 to 29), so the 4 targets save about 2 STM.

logs are read for their literal scramble twists; scrambles given only by seed are rejected. stm is counted the way hsc2 counts it (runs of consecutive twists on the same cell), which matches the `stmN` in hsc2's log filenames.

## ref

- [flat hypercube](https://github.com/milojacquet/flat-hypercube)
- [robodoan](https://github.com/HactarCE/robodoan)
- [hsearch](https://github.com/HactarCE/hsearch)
