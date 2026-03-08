# Release Notes

Current: v1-04

## Changes

### v1-04 260308
+ Added rayon pthread parallelisation option (use --parallel in the command line). Gives a
  speedup of ~1.7x, noticeable on large population sizes

### v1-03 260308
+ Added support to show most common tape AND most common replicator (with instruction code),
  together with hex byes, since it is plausible that the most common tape has no instruction code

### v1-02 260306 
+ bff.rs:32-40 — Head positions read from tape[0] and tape[1], IP starts at 2 (matching paper's 
  BFF_HEADS variant). Tapes shorter than 3 bytes are now rejected early. Tests rewritten accordingly.
+ soup.rs:103-124 — Mutation moved inside the pair loop: concatenate → mutate → execute → split. Old 
  apply_mutations method removed.
+ spatial.rs:136-152 — Same mutation-before-execution change applied.

### v1-01 260306 
+ Initial implementation: Suggests updating the replicator compression check as the current
  implementation is quite coarse and might miss state transitions
+ Set seed at start if not passed as a commnd line arg, so that it is user visible for recreation of simulation runs
+ Added "unique tapes" metric to give more interpretatbility at the tape and not byte/sumbol level

