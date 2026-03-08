// =============================================================================
// Complexity Metrics — Detecting the pre-life → life state transition
// =============================================================================
//
// The paper introduces "high-order entropy" as a complexity metric:
//
//   high_order_entropy = shannon_entropy - normalized_kolmogorov_complexity
//
// Intuitively:
//   - Random noise has high Shannon entropy but also high Kolmogorov complexity
//     → high-order entropy ≈ 0 (no structure beyond randomness)
//   - A soup dominated by copies of one replicator has moderate Shannon entropy
//     (the replicator uses a non-uniform subset of bytes) but LOW Kolmogorov
//     complexity (it's highly compressible — many copies of the same pattern)
//     → high-order entropy > 0 (real structure exists)
//
// The paper approximates Kolmogorov complexity using brotli compression.
// We use a simple run-length + LZ77-style estimate since we can't depend on
// external compression libraries. The exact value doesn't matter as much as
// detecting the *transition* from ~0 to >1.

/// Statistics for a soup at a given epoch
#[derive(Debug, Clone)]
pub struct SoupStats {
    /// Shannon entropy of the byte distribution across all tapes (bits)
    pub shannon_entropy: f64,
    /// High-order entropy approximation (Shannon - compression ratio)
    pub high_order_entropy: f64,
    /// Number of distinct byte values present in the soup
    pub unique_bytes: usize,
    /// Fraction of the soup occupied by the single most common byte
    pub top_token_fraction: f64,
    /// Number of distinct tapes in the soup
    pub unique_tapes: usize,
}

/// Compute Shannon entropy (in bits) of a byte frequency distribution
pub fn shannon_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut counts = [0u64; 256];
    for &b in data {
        counts[b as usize] += 1;
    }
    let n = data.len() as f64;
    let mut entropy = 0.0;
    for &c in &counts {
        if c > 0 {
            let p = c as f64 / n;
            entropy -= p * p.log2();
        }
    }
    entropy
}

/// Count unique byte values in data
pub fn unique_byte_count(data: &[u8]) -> usize {
    let mut seen = [false; 256];
    for &b in data {
        seen[b as usize] = true;
    }
    seen.iter().filter(|&&s| s).count()
}

/// Find the fraction of data occupied by the most common byte value
pub fn top_token_fraction(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut counts = [0u64; 256];
    for &b in data {
        counts[b as usize] += 1;
    }
    let max_count = *counts.iter().max().unwrap();
    max_count as f64 / data.len() as f64
}

/// Approximate the "normalized Kolmogorov complexity" of data using brotli
/// compression, following the paper's approach.
///
/// Returns compressed bits per original byte — a value between ~0 (perfectly
/// compressible) and ~8 (incompressible), matching Shannon entropy units.
pub fn compression_ratio_estimate(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }

    let mut compressed = Vec::new();
    let params = brotli::enc::BrotliEncoderParams {
        quality: 5,
        ..Default::default()
    };

    brotli::BrotliCompress(&mut &data[..], &mut compressed, &params)
        .expect("brotli compression failed");

    (compressed.len() as f64 * 8.0) / data.len() as f64
}

/// Compute high-order entropy: Shannon entropy - compression ratio.
/// Values near 0 indicate random noise; values > 1 indicate structured content
/// (like self-replicators that have taken over the soup).
pub fn high_order_entropy(data: &[u8]) -> f64 {
    let se = shannon_entropy(data);
    let cr = compression_ratio_estimate(data);
    // Clamp to non-negative (the approximation can sometimes overshoot)
    (se - cr).max(0.0)
}

/// Count the number of distinct tapes in the soup (by exact match)
pub fn count_unique_tapes(tapes: &[Vec<u8>]) -> usize {
    let mut seen = std::collections::HashSet::new();
    for tape in tapes {
        seen.insert(tape.as_slice());
    }
    seen.len()
}

/// Compute all soup statistics at once
pub fn compute_stats(all_data: &[u8], tapes: &[Vec<u8>]) -> SoupStats {
    SoupStats {
        shannon_entropy: shannon_entropy(all_data),
        high_order_entropy: high_order_entropy(all_data),
        unique_bytes: unique_byte_count(all_data),
        top_token_fraction: top_token_fraction(all_data),
        unique_tapes: count_unique_tapes(tapes),
    }
}

/// Find the most common contiguous tape-sized pattern in the soup.
/// Returns the tape that appears most frequently (by exact match).
pub fn find_most_common_tape(tapes: &[Vec<u8>]) -> Vec<u8> {
    if tapes.is_empty() {
        return vec![];
    }
    let mut counts: std::collections::HashMap<&[u8], usize> = std::collections::HashMap::new();
    for tape in tapes {
        *counts.entry(tape.as_slice()).or_insert(0) += 1;
    }
    let best = counts.into_iter().max_by_key(|&(_, count)| count).unwrap();
    best.0.to_vec()
}

/// Find the most common tape that contains at least `min_instructions` BFF
/// instruction bytes. Returns None if no tape meets the threshold.
pub fn find_most_common_replicator_tape(tapes: &[Vec<u8>], min_instructions: usize) -> Option<Vec<u8>> {
    let mut counts: std::collections::HashMap<&[u8], usize> = std::collections::HashMap::new();
    for tape in tapes {
        let bff_count = tape.iter().filter(|&&b| {
            matches!(b, b'<' | b'>' | b'{' | b'}' | b'+' | b'-' | b'.' | b',' | b'[' | b']')
        }).count();
        if bff_count >= min_instructions {
            *counts.entry(tape.as_slice()).or_insert(0) += 1;
        }
    }
    counts.into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(tape, _)| tape.to_vec())
}
