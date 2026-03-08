// =============================================================================
// Spatial Soup — 2D grid BFF simulation (Section 2.2 of the paper)
// =============================================================================
//
// In this mode, tapes are arranged on a 2D grid and can only interact with
// neighbors within a Chebyshev distance of `radius` (default 2).
//
// This produces qualitatively different dynamics:
//   - Self-replicators spread as visible wavefronts across the grid
//   - Multiple replicator species can coexist and compete for territory
//   - The takeover time scales as O(√n) rather than O(log n)
//
// ALGORITHM (each epoch):
//   1. Iterate through all grid positions in random order
//   2. For each position P (not yet "taken" this epoch):
//      a. Select a random neighbor N within the radius
//      b. If N is also not taken, mark both as taken
//      c. Concatenate P++N, execute BFF, split back
//   3. Apply background mutations to all tapes

use rand::prelude::*;
use rand::rngs::StdRng;

use crate::bff;
use crate::metrics::{self, SoupStats};

pub struct SpatialSoup {
    /// Grid of tapes (row-major: index = y * width + x)
    tapes: Vec<Vec<u8>>,
    width: usize,
    height: usize,
    tape_len: usize,
    max_steps: usize,
    mutation_rate: f64,
    radius: usize,
    rng: StdRng,
    pub epoch: usize,
    exec_buffer: Vec<u8>,
}

impl SpatialSoup {
    pub fn new(
        width: usize,
        height: usize,
        tape_len: usize,
        max_steps: usize,
        mutation_rate: f64,
        radius: usize,
        seed: u64,
    ) -> Self {
        let mut rng = if seed == 0 {
            StdRng::from_entropy()
        } else {
            StdRng::seed_from_u64(seed)
        };

        let n = width * height;
        let tapes: Vec<Vec<u8>> = (0..n)
            .map(|_| {
                let mut tape = vec![0u8; tape_len];
                rng.fill_bytes(&mut tape);
                tape
            })
            .collect();

        let exec_buffer = vec![0u8; tape_len * 2];

        SpatialSoup {
            tapes,
            width,
            height,
            tape_len,
            max_steps,
            mutation_rate,
            radius,
            rng,
            epoch: 0,
            exec_buffer,
        }
    }

    /// Get grid neighbors of position (x, y) within Chebyshev distance `radius`.
    fn neighbors(&self, x: usize, y: usize) -> Vec<usize> {
        let r = self.radius as isize;
        let mut result = Vec::new();
        for dy in -r..=r {
            for dx in -r..=r {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = x as isize + dx;
                let ny = y as isize + dy;
                if nx >= 0 && nx < self.width as isize && ny >= 0 && ny < self.height as isize {
                    result.push(ny as usize * self.width + nx as usize);
                }
            }
        }
        result
    }

    pub fn step(&mut self) {
        self.epoch += 1;
        let n = self.tapes.len();

        // Random iteration order
        let mut order: Vec<usize> = (0..n).collect();
        order.shuffle(&mut self.rng);

        // Track which tapes have been "taken" this epoch
        let mut taken = vec![false; n];

        // Collect pairs first, then execute (to satisfy borrow checker)
        let mut pairs: Vec<(usize, usize)> = Vec::new();

        for &idx in &order {
            if taken[idx] {
                continue;
            }

            let x = idx % self.width;
            let y = idx / self.width;
            let neighbors = self.neighbors(x, y);

            // Filter to untaken neighbors and pick one at random
            let available: Vec<usize> = neighbors.into_iter().filter(|&n| !taken[n]).collect();
            if available.is_empty() {
                continue;
            }

            let neighbor_idx = available[self.rng.gen_range(0..available.len())];
            taken[idx] = true;
            taken[neighbor_idx] = true;
            pairs.push((idx, neighbor_idx));
        }

        // Execute all interactions
        let tl = self.tape_len;
        for (i, j) in pairs {
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

            bff::execute(&mut self.exec_buffer, self.max_steps);

            self.tapes[i].copy_from_slice(&self.exec_buffer[..tl]);
            self.tapes[j].copy_from_slice(&self.exec_buffer[tl..tl * 2]);
        }
    }

    pub fn compute_stats(&self) -> SoupStats {
        let all_data: Vec<u8> = self.tapes.iter().flat_map(|t| t.iter().copied()).collect();
        metrics::compute_stats(&all_data, &self.tapes)
    }
}
