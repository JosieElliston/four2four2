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

## phase 2 (reduced moveset)

god's number for phase 2 under the reduced moveset is 21 STM, taken from the pruning table. here's the (exact) histogram of STM from a random state:

TODO: histogram

and here's an example of an antipode, none of which are geometrically symmetric:

TODO: look harder for symmetric ones, like having U or D or I or O symmetric rather than the entire state

TODO: log file
TODO: screenshot

## phase 2 (full moveset)

to find god's number for phase 2 under the full moveset, we take the phase 2 states and twist upperbounds found in the reduce moveset, then iteratively improve the worst states by 1 STM with the optimal solver, until we find a state we can't improve.

except i only got down to 15 STM bc 33.07M states is a bit much.

this gives god's number for phase 2 under the full moveset as <= 15 STM.

## full puzzle

### empirically

here's the histogram of the optimal STM for TODO:number random states:

### analytically

the state space and number of twists gives a combinatorial lower bound of TODO STM.

god's number for the two phases gives an upper bound of 10 + 15 = 25 STM.

TODO: try the phase 2 antipode to improve the combinatorial lower bound
TODO: try the antipode alg from discord

