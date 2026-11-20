# antipodes / god's number

we fully understand each of the phases; we have bounds for the full puzzle

## phase 1

god's number for phase 1 is 10 STM, taken from the pruning table. here's the (exact) histogram of STM from a random state:

TODO: histogram

and here's an example of an antipode, chosen for having a nice solution, alternating `ε`-cell twists and `ηD_εA_εD`:

```
εC_εD ηD_εA_εD
εD_εC ηD_εA_εD
εC_εD ηD_εA_εD
εC_εD ηD_εA_εD
εD_εA εD_εA ηD_εA_εD
```

TODO: invert the solution so it's `ηD_εA_εD` alternating with `ε` and the `ηD_εA_εD` line up
also research with grabbing all optimal solutions, i don't like `εD_εA εD_εA` is duplicated

TODO: log file
TODO: screenshot

the combinatorial lower bound is 7 STM: 1.05e8 positions.

## phase 2 (reduced moveset)

god's number for phase 2 under the reduced moveset is 21 STM, taken from the pruning table. here's the (exact) histogram of STM from a random state:

TODO: histogram

and here's an example of an antipode, none of which are geometrically symmetric:

TODO: look harder for symmetric ones, like having U or D or I or O symmetric rather than the entire state

TODO: log file
TODO: screenshot

the combinatorial lower bound is 11 STM: 1.02e8 positions, 12 twists.

## phase 2 (full moveset)

god's number for phase 2 under the full moveset is 14 STM. the method is the one Rokicki used for the 3x3 (one coset, here the phase 2 set H itself), plus symmetry:

1. **enumerate excursions.** DFS with the full moveset from solved to H, up to T = 12 twists, pruned by the phase 1 table, keeping words that land in H and end with a twist outside the reduced moveset, taking the shortest word for each H state. 2.6e11 nodes, 21 min.
2. **BFS over H.** start from those lengths and BFS with the reduced moveset. this gives a real solution (an upper bound) for all H states. 8 s.
3. **the leftovers.** 75,584 states have an upper bound above 14. reduce them under the 64 symmetries that preserve H (below), get 2,262 orbits. run the optimal solver with `--max 14` on one state per orbit. all of them solve in at most 14. 5 min.
4. **tightness.** the optimal solver finds states that take exactly 14, eg P2 index 110246436.

TODO: log file
TODO: screenshot

to rerun, with `cargo build --release --examples` first:

```
target/release/examples/coset 12 /tmp/coset12.bin     # steps 1 and 2
target/release/examples/residual /tmp/coset12.bin 14  # step 3
```

the symmetries: conjugate by any of the 16 sign flips, optionally composed with the x↔y swap, then rotate the whole puzzle to bring piece 0 back home. that maps H onto H and keeps STM. inversion doubles the group to 64. (the symmetries that fix slot 0 directly are just identity and x↔y.)

the cost grows about 10x per unit of T (T=11: 2 min, 1.28M leftovers). so T=12 plus solving the leftovers is much cheaper than T=14 alone, and also much cheaper than solving all 101.6M states one at a time.

exact histogram up to 12 STM. the remaining 46,252,268 states take 13 or 14, and splitting them needs T = 13 (about 3.5 h):

| STM | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| states | 1 | 12 | 57 | 219 | 770 | 2,481 | 9,588 | 45,020 | 209,316 | 858,878 | 3,400,098 | 13,026,096 | 37,801,616 |

the combinatorial lower bound is 7 STM: the same positions as reduced phase 2, but with all 28 twists.

## full puzzle

### empirically

here's the histogram of the optimal STM for TODO:number random states:

### analytically

the state space and number of twists gives a combinatorial lower bound of 13 STM: 15!·2¹³ ≈ 1.07e16 positions, but fewer than that many twist sequences of length at most 12 (there are 2.7e16 of length at most 13).

each combinatorial bound is the least n such that there are at least as many sequences of length ≤ n as positions. the sequences counted never twist the same cell twice in a row, and for each set of two-twist sequences with the same product, only one is counted. every position still has a shortest solution among them. `cargo run --release --example counting` prints all four.

the exact phase 2 histogram above shows how much this underestimates: counting gives 7, the truth is 14.

god's number for the two phases gives an upper bound for the full puzzle of 10 + 14 = 24 STM.

TODO: try the phase 2 antipode to improve the combinatorial lower bound
TODO: try the antipode alg from discord
