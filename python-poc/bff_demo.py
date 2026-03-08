#!/usr/bin/env python3
"""
BFF Primordial Soup — Python reference implementation.

A faithful implementation of the BFF experiment from:
"Computational Life: How Well-formed, Self-replicating Programs Emerge
from Simple Interaction" (Agüera y Arcas et al., 2024)

This is a slower but runnable reference. For performance, use the Rust version.
"""

import random
import math
import sys
from collections import Counter

# ── BFF Instruction Set ──────────────────────────────────────────────────────
# Byte values for the 10 BFF instructions
INST = {
    ord('<'): 'head0_left',
    ord('>'): 'head0_right',
    ord('{'): 'head1_left',
    ord('}'): 'head1_right',
    ord('-'): 'dec',
    ord('+'): 'inc',
    ord('.'): 'copy_to_head1',
    ord(','): 'copy_to_head0',
    ord('['): 'loop_start',
    ord(']'): 'loop_end',
}

def find_matching_close(tape, pos):
    """Find matching ] for [ at pos."""
    depth = 1
    i = pos + 1
    while i < len(tape):
        if tape[i] == ord('['):
            depth += 1
        elif tape[i] == ord(']'):
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return None

def find_matching_open(tape, pos):
    """Find matching [ for ] at pos."""
    depth = 1
    i = pos - 1
    while i >= 0:
        if tape[i] == ord(']'):
            depth += 1
        elif tape[i] == ord('['):
            depth -= 1
            if depth == 0:
                return i
        i -= 1
    return None

def execute_bff(tape, max_steps=8192):
    """Execute BFF on a mutable tape (bytearray). Returns steps executed."""
    n = len(tape)
    if n == 0:
        return 0

    ip = 0
    head0 = 0
    head1 = 0
    steps = 0

    while steps < max_steps and ip < n:
        instr = tape[ip]

        if instr == ord('<'):
            head0 = (head0 - 1) % n
        elif instr == ord('>'):
            head0 = (head0 + 1) % n
        elif instr == ord('{'):
            head1 = (head1 - 1) % n
        elif instr == ord('}'):
            head1 = (head1 + 1) % n
        elif instr == ord('-'):
            tape[head0] = (tape[head0] - 1) & 0xFF
        elif instr == ord('+'):
            tape[head0] = (tape[head0] + 1) & 0xFF
        elif instr == ord('.'):
            tape[head1] = tape[head0]
        elif instr == ord(','):
            tape[head0] = tape[head1]
        elif instr == ord('['):
            if tape[head0] == 0:
                m = find_matching_close(tape, ip)
                if m is None:
                    break
                ip = m
        elif instr == ord(']'):
            if tape[head0] != 0:
                m = find_matching_open(tape, ip)
                if m is None:
                    break
                ip = m
        # else: no-op

        ip += 1
        steps += 1

    return steps


# ── Complexity Metrics ────────────────────────────────────────────────────────

def shannon_entropy(data):
    """Shannon entropy in bits of a byte sequence."""
    if not data:
        return 0.0
    counts = Counter(data)
    n = len(data)
    return -sum((c / n) * math.log2(c / n) for c in counts.values())

def unique_bytes(data):
    return len(set(data))

def top_token_fraction(data):
    if not data:
        return 0.0
    counts = Counter(data)
    return counts.most_common(1)[0][1] / len(data)

def compression_ratio_estimate(data):
    """Simple 4-gram uniqueness compression proxy."""
    if len(data) < 4:
        return shannon_entropy(data)
    ngrams = set()
    for i in range(len(data) - 3):
        ngrams.add((data[i], data[i+1], data[i+2], data[i+3]))
    ratio = len(ngrams) / (len(data) - 3)
    return ratio * 8.0

def high_order_entropy(data):
    se = shannon_entropy(data)
    cr = compression_ratio_estimate(data)
    return max(0.0, se - cr)


# ── Primordial Soup ──────────────────────────────────────────────────────────

class Soup:
    def __init__(self, soup_size, tape_len=64, max_steps=8192, mutation_rate=0.00024, seed=None):
        self.tape_len = tape_len
        self.max_steps = max_steps
        self.mutation_rate = mutation_rate
        self.rng = random.Random(seed)
        self.epoch = 0

        # Initialize tapes with uniform random bytes
        self.tapes = [
            bytearray(self.rng.getrandbits(8) for _ in range(tape_len))
            for _ in range(soup_size)
        ]

    def step(self):
        """Execute one epoch."""
        self.epoch += 1
        n = len(self.tapes)
        tl = self.tape_len

        # Shuffle indices and pair them
        indices = list(range(n))
        self.rng.shuffle(indices)

        for k in range(0, n - 1, 2):
            i, j = indices[k], indices[k + 1]

            # Concatenate
            combined = bytearray(self.tapes[i] + self.tapes[j])

            # Execute
            execute_bff(combined, self.max_steps)

            # Split back
            self.tapes[i][:] = combined[:tl]
            self.tapes[j][:] = combined[tl:tl * 2]

        # Background mutations
        if self.mutation_rate > 0:
            for tape in self.tapes:
                for pos in range(len(tape)):
                    if self.rng.random() < self.mutation_rate:
                        tape[pos] = self.rng.getrandbits(8)

    def flat_data(self):
        return bytearray(b for t in self.tapes for b in t)

    def stats(self):
        data = self.flat_data()
        return {
            'entropy': shannon_entropy(data),
            'hi_order': high_order_entropy(data),
            'unique': unique_bytes(data),
            'top_frac': top_token_fraction(data),
        }


def format_tape(tape):
    """Pretty-print a tape."""
    chars = []
    for b in tape:
        if b in INST:
            chars.append(chr(b))
        elif b == 0:
            chars.append('0')
        else:
            chars.append('·')
    return ''.join(chars)


# ── Main ─────────────────────────────────────────────────────────────────────

def main():
    # Parameters — smaller than the paper's defaults for a quick demo,
    # but large enough to observe the dynamics.
    SOUP_SIZE = 1024       # Paper uses 2^17 = 131072; we use 512 for speed
    TAPE_LEN  = 64        # Same as paper
    MAX_STEPS = 8192      # Same as paper (2^13)
    MUT_RATE  = 0.00024   # Same as paper (0.024%)
    EPOCHS    = 5000      # Paper runs 16000; we use 5000 for demo
    REPORT    = 50        # Report every N epochs
    SEED      = None      # Set to int for reproducibility

    if len(sys.argv) > 1:
        SEED = int(sys.argv[1])

    print("╔══════════════════════════════════════════════════════════════╗")
    print("║  BFF Primordial Soup — Python Reference Implementation      ║")
    print("║  After Agüera y Arcas et al. (2024), arXiv:2406.19108       ║")
    print("╚══════════════════════════════════════════════════════════════╝")
    print()
    print(f"  Soup size:     {SOUP_SIZE} tapes")
    print(f"  Tape length:   {TAPE_LEN} bytes")
    print(f"  Max steps:     {MAX_STEPS}")
    print(f"  Mutation rate: {MUT_RATE} ({MUT_RATE*100:.4f}%)")
    print(f"  Epochs:        {EPOCHS}")
    print(f"  Seed:          {SEED if SEED is not None else 'random'}")
    print()
    print(f"{'epoch':>8} {'entropy':>10} {'hi-order':>10} {'unique':>8} {'top_tok%':>10}  status")
    print("-" * 68)

    soup = Soup(SOUP_SIZE, TAPE_LEN, MAX_STEPS, MUT_RATE, SEED)
    transitioned = False

    for epoch in range(1, EPOCHS + 1):
        soup.step()

        if epoch % REPORT == 0 or epoch == 1:
            s = soup.stats()
            status = ""
            if s['hi_order'] >= 1.0 and not transitioned:
                transitioned = True
                status = " *** STATE TRANSITION ***"

                # Show the most common tape
                tape_counts = Counter(bytes(t) for t in soup.tapes)
                most_common = bytearray(tape_counts.most_common(1)[0][0])
                count = tape_counts.most_common(1)[0][1]
                print(f"\n  Dominant tape ({count}/{SOUP_SIZE} copies):")
                print(f"  [{format_tape(most_common)}]\n")
            elif transitioned:
                status = " [post-transition]"

            print(f"{epoch:>8} {s['entropy']:>10.4f} {s['hi_order']:>10.4f} "
                  f"{s['unique']:>8} {s['top_frac']*100:>9.2f}%  {status}")

    print()
    if transitioned:
        print("✓ Self-replicators emerged spontaneously!")
    else:
        print("✗ No state transition in this run (expected ~60% of the time with small soup).")
        print("  Try again, increase soup size, or increase epochs.")
    print()

if __name__ == "__main__":
    main()
