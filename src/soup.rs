// =============================================================================
// Primordial Soup — Well-mixed (0D) BFF simulation
// =============================================================================
//
// This implements the core "Turing gas" variant from the paper (Section 2.1).
//
// ALGORITHM (each epoch):
//   1. Randomly shuffle all tape indices into pairs: (A₀,B₀), (A₁,B₁), ...
//   2. For each pair (A, B):
//      a. Concatenate: combined = A ++ B  (length = 2 * tape_len)
//      b. Execute BFF on `combined` for up to max_steps
//      c. Split: A' = combined[0..tape_len], B' = combined[tape_len..2*tape_len]
//      d. Replace A and B with A' and B'
//   3. Apply background mutations (flip random bytes at mutation_rate)
//
// The paper describes this as an irreversible chemical reaction:
//   A + B → split(exec(A++B)) = A' + B'
//
// Note: The paper also considers both orderings (AB and BA) but shows that
// using just one ordering per epoch is sufficient for self-replicators to emerge.

use rand::prelude::*;
use rand::rngs::StdRng;
use rayon::prelude::*;

use crate::bff;
use crate::metrics::{self, SoupStats};

pub struct Soup {
    /// The pool of tapes
    tapes: Vec<Vec<u8>>,
    /// Length of each individual tape
    tape_len: usize,
    /// Maximum BFF execution steps per interaction
    max_steps: usize,
    /// Background mutation rate (probability per byte per epoch)
    mutation_rate: f64,
    /// Random number generator
    rng: StdRng,
    /// Current epoch counter
    pub epoch: usize,
    /// Reusable buffer for concatenated execution
    exec_buffer: Vec<u8>,
}

impl Soup {
    /// Create a new primordial soup with randomly initialized tapes.
    pub fn new(
        soup_size: usize,
        tape_len: usize,
        max_steps: usize,
        mutation_rate: f64,
        seed: u64,
    ) -> Self {
        // TODO: seed == 0 is not possible given thar it is now initialised in main. Deprecate
        let mut rng = if seed == 0 {
            StdRng::from_entropy()
        } else {
            StdRng::seed_from_u64(seed)
        };

        // Initialize all tapes with uniform random bytes
        let tapes: Vec<Vec<u8>> = (0..soup_size)
            .map(|_| {
                let mut tape = vec![0u8; tape_len];
                rng.fill_bytes(&mut tape);
                tape
            })
            .collect();

        let exec_buffer = vec![0u8; tape_len * 2];

        Soup {
            tapes,
            tape_len,
            max_steps,
            mutation_rate,
            rng,
            epoch: 0,
            exec_buffer,
        }
    }

    /// Execute one epoch of the primordial soup simulation.
    pub fn step(&mut self) {
        self.epoch += 1;
        let n = self.tapes.len();
        if n < 2 {
            return;
        }

        // Step 1: Create a random pairing of all tapes
        // We shuffle indices and pair consecutive elements
        let mut indices: Vec<usize> = (0..n).collect();
        indices.shuffle(&mut self.rng);

        // Step 2: For each pair, concatenate → execute → split
        let pairs: Vec<(usize, usize)> = indices
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| (c[0], c[1]))
            .collect();

        for (i, j) in pairs {
            // Concatenate tape[i] ++ tape[j]
            let tl = self.tape_len;
            self.exec_buffer[..tl].copy_from_slice(&self.tapes[i]);
            self.exec_buffer[tl..tl * 2].copy_from_slice(&self.tapes[j]);

            // Mutate the concatenated tape before execution (matches paper)
            if self.mutation_rate > 0.0 {
                for byte in self.exec_buffer.iter_mut() {
                    if self.rng.gen::<f64>() < self.mutation_rate {
                        *byte = self.rng.gen();
                    }
                }
            }

            // Execute BFF on the concatenated tape
            bff::execute(&mut self.exec_buffer, self.max_steps);

            // Split back and update
            self.tapes[i].copy_from_slice(&self.exec_buffer[..tl]);
            self.tapes[j].copy_from_slice(&self.exec_buffer[tl..tl * 2]);
        }
    }

    /// Execute one epoch using Rayon parallel execution for BFF processing.
    ///
    /// Shuffling and mutation are done sequentially (to preserve RNG determinism
    /// for a given seed), but BFF execution of all pairs runs in parallel.
    /// Same seed produces identical results whether using step() or step_parallel().
    pub fn step_parallel(&mut self) {
        self.epoch += 1;
        let n = self.tapes.len();
        if n < 2 {
            return;
        }

        // Step 1: Create a random pairing (sequential — uses self.rng)
        let mut indices: Vec<usize> = (0..n).collect();
        indices.shuffle(&mut self.rng);

        let pairs: Vec<(usize, usize)> = indices
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| (c[0], c[1]))
            .collect();

        // Step 2: Concatenate and mutate into per-pair buffers (sequential — uses self.rng)
        let tl = self.tape_len;
        let mut buffers: Vec<Vec<u8>> = pairs.iter().map(|&(i, j)| {
            let mut buf = vec![0u8; tl * 2];
            buf[..tl].copy_from_slice(&self.tapes[i]);
            buf[tl..tl * 2].copy_from_slice(&self.tapes[j]);

            if self.mutation_rate > 0.0 {
                for byte in buf.iter_mut() {
                    if self.rng.gen::<f64>() < self.mutation_rate {
                        *byte = self.rng.gen();
                    }
                }
            }
            buf
        }).collect();

        // Step 3: Execute BFF on all pairs in parallel
        buffers.par_iter_mut().for_each(|buf| {
            bff::execute(buf, self.max_steps);
        });

        // Step 4: Split results back into tapes (sequential)
        for (idx, &(i, j)) in pairs.iter().enumerate() {
            self.tapes[i].copy_from_slice(&buffers[idx][..tl]);
            self.tapes[j].copy_from_slice(&buffers[idx][tl..tl * 2]);
        }
    }

    /// Compute complexity statistics for the current state of the soup.
    pub fn compute_stats(&self) -> SoupStats {
        let all_data: Vec<u8> = self.tapes.iter().flat_map(|t| t.iter().copied()).collect();
        metrics::compute_stats(&all_data, &self.tapes)
    }

    /// Get the most frequently occurring tape (exact match).
    /// Useful for inspecting what the dominant replicator looks like.
    pub fn get_most_common_tape(&self) -> Vec<u8> {
        metrics::find_most_common_tape(&self.tapes)
    }

    /// Get the most common tape containing at least `min_instructions` BFF instructions.
    pub fn get_most_common_replicator_tape(&self, min_instructions: usize) -> Option<Vec<u8>> {
        metrics::find_most_common_replicator_tape(&self.tapes, min_instructions)
    }

    /// Get a reference to all tapes (for external analysis).
    pub fn tapes(&self) -> &[Vec<u8>] {
        &self.tapes
    }
}
