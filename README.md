# BFF-Replicator

This project is a pure Rust implementation of the BFF (Brainfuck Family) primordial soup experiment from Google Research:

> **"Computational Life: How Well-formed, Self-replicating Programs Emerge from Simple Interaction"**
> Agüera y Arcas et al. (2024) — [arXiv:2406.19108](https://arxiv.org/abs/2406.19108)

Note that this was largely derived from the paper, so there may be differences with the original 
implementation (which includes a CUDA option).

This project provides a (optional) RAYON-based parallel threaded implementation of a
self-modifying soup of BF programs. This code demonstrate emergence of self-replicators. 

The paper reports that phase transitions occur ~40% of the time in test runs, however
I have not yet run enough experiments to verify this metric. In practice phase transitions
don't always happen, and depend on the seed, and the size (diversity) of the soup size.
Larger populations give a higher chance of transitions.

## Run instructions

You can then run a simulation with

  `./bff-replicator --soup-size 131072 --parallel --seed 1567861621370340275`
  
to replicate the paper conditions. The seed used here did trigger replication but there is
no guarantee this will happen on your machine (as the RND generator will differ).

If seed is omitted then a random seed will be chosen - but will be printed to the screen 
in case you want to replicate the experiment later. If parallel is omitted then the code
will run single threaded. E.g.,

  `./bff-replicator --soup-size 131072`

## What This Demonstrates

Self-replicating programs emerge *spontaneously* from a pool of random byte strings — with **no fitness function**, **no selection pressure**, and **no pre-designed replicators**. The only ingredients are:

1. A simple programming language (BFF — extended Brainfuck)
2. Random interactions between programs
3. Time

This supports the paper's central thesis: **computation is a dynamical attractor**. In any substrate capable of supporting computation, self-replicators tend to arise because replication is a kinetically stable state.

## The BFF Instruction Set

BFF extends Brainfuck to be fully self-contained (no I/O streams). Instructions and data share the same tape (Von Neumann architecture), which is critical — it allows programs to modify themselves and each other.

| Instruction | Effect |
|---|---|
| `<` / `>` | Move head0 left / right |
| `{` / `}` | Move head1 left / right |
| `+` / `-` | Increment / decrement `tape[head0]` |
| `.` | Copy: `tape[head1] = tape[head0]` |
| `,` | Copy: `tape[head0] = tape[head1]` |
| `[` | If `tape[head0] == 0`, jump forward to matching `]` |
| `]` | If `tape[head0] != 0`, jump back to matching `[` |

Only 10 of 256 possible byte values are instructions. The rest are no-ops (inert data). This means ~96% of random bytes do nothing — but the 4% that are instructions can create meaningful computation.

## How the Simulation Works

```
┌─────────────────────────────────────────────────┐
│  PRIMORDIAL SOUP (N random tapes, each 64 bytes)│
│  ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐      │
│  │rand│ │rand│ │rand│ │rand│ │rand│ │rand│ ...  │
│  └────┘ └────┘ └────┘ └────┘ └────┘ └────┘      │
└─────────────────────────────────────────────────┘
                    │
         Each epoch │
                    ▼
  1. Randomly pair tapes:  (A, B)
  2. Concatenate:          combined = A ++ B
  3. Execute BFF:          exec(combined, max_steps=8192)
  4. Split:                A' = combined[0:64], B' = combined[64:128]
  5. Replace originals:    A ← A', B ← B'
  6. Apply mutations:      (optional, default 0.024% per byte)
                    │
                    ▼
  After ~2000-5000 epochs, a PHASE TRANSITION often occurs:
  - High-order entropy jumps from ~0 to >1
  - Unique token count drops sharply
  - One or more self-replicators dominate the soup
```

## Building and Running

```bash
# Build in release mode (important for performance!)
cargo build --release

# Run with defaults (4096 tapes, 64 bytes each, 16000 epochs)
cargo run --release

# Smaller soup for faster experimentation
cargo run --release -- --soup-size 1024 --epochs 8000

# No mutations (self-replicators still emerge via self-modification!)
cargo run --release -- --mutation-rate 0.0

# 2D spatial mode (replicator wavefronts!)
cargo run --release -- --mode spatial --width 60 --height 34

# Reproducible run with fixed seed
cargo run --release -- --seed 42
```

## What to Look For

When running, watch for the **state transition**:

```
   epoch    entropy   hi-order     unique   top_token%  status
------------------------------------------------------------------------
     100     7.9993     0.0012        256        0.49%
     200     7.9991     0.0015        256        0.50%
     ...
    2400     7.8234     0.0089        256        0.62%
    2500     6.1247     2.3401        198       4.82%   *** STATE TRANSITION ***
    2600     5.8912     2.8734        187       5.41%   [post-transition]
```

- **Before**: Shannon entropy ≈ 8 (maximum randomness), high-order entropy ≈ 0
- **After**: Shannon entropy drops, high-order entropy jumps above 1, unique tokens decrease


## References

- [Paper (arXiv)](https://arxiv.org/abs/2406.19108)
- [Sean Carroll podcast discussion](https://www.preposterousuniverse.com/podcast/2024/08/19/286-blaise-aguera-y-arcas-on-the-emergence-of-replication-and-computation/)
- Agüera y Arcas, *What Is Life?: Evolution as Computation* (MIT Press, 2025)
