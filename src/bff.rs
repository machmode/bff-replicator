// =============================================================================
// BFF Interpreter — Extended Brainfuck for self-modifying programs
// =============================================================================
//
// This implements the BFF instruction set as described in Section 2 of the paper.
//
// KEY DESIGN DECISIONS (from the paper):
//
// 1. Von Neumann architecture: instructions and data share the same tape.
//    This is critical — it means programs can modify themselves and each other,
//    which is the mechanism by which self-replicators emerge.
//
// 2. Two data heads (head0, head1) replace stdin/stdout from standard Brainfuck.
//    The '.' instruction copies from head0 to head1 (write).
//    The ',' instruction copies from head1 to head0 (read).
//    This allows one half of a concatenated pair to write into the other half.
//
// 3. Programs terminate after a fixed number of steps (default 8192 = 2^13),
//    or when an unmatched bracket is encountered.
//
// 4. All 256 byte values are valid tape contents, but only 10 are instructions.
//    The remaining 245 values are no-ops (and can serve as inert data).
//    This means ~96% of random bytes are no-ops, which is important: it means
//    random tapes mostly do nothing, but occasionally execute meaningful code.

/// Execute BFF code on a mutable tape.
///
/// The tape represents two concatenated programs (A ++ B). After execution,
/// the tape is modified in-place — both "programs" may have been altered.
///
/// Returns the number of steps actually executed (useful for diagnostics).
pub fn execute(tape: &mut [u8], max_steps: usize) -> usize {
    let len = tape.len();
    if len < 3 {
        return 0;
    }

    // Per the paper's BFF_HEADS variant: the first two bytes encode the initial
    // head positions, and execution begins at byte 2.
    let mut head0: usize = (tape[0] as usize) % len;
    let mut head1: usize = (tape[1] as usize) % len;
    let mut ip: usize = 2;
    let mut steps: usize = 0;

    while steps < max_steps && ip < len {
        let instr = tape[ip];
        match instr {
            // head0 movement
            b'<' => {
                head0 = if head0 == 0 { len - 1 } else { head0 - 1 };
            }
            b'>' => {
                head0 = (head0 + 1) % len;
            }

            // head1 movement
            b'{' => {
                head1 = if head1 == 0 { len - 1 } else { head1 - 1 };
            }
            b'}' => {
                head1 = (head1 + 1) % len;
            }

            // Arithmetic on tape[head0]
            b'-' => {
                tape[head0] = tape[head0].wrapping_sub(1);
            }
            b'+' => {
                tape[head0] = tape[head0].wrapping_add(1);
            }

            // Copy: tape[head1] = tape[head0]  (write from head0 to head1)
            b'.' => {
                tape[head1] = tape[head0];
            }

            // Copy: tape[head0] = tape[head1]  (read from head1 into head0)
            b',' => {
                tape[head0] = tape[head1];
            }

            // Loop start: if tape[head0] == 0, jump forward past matching ']'
            b'[' => {
                if tape[head0] == 0 {
                    match find_matching_close(tape, ip) {
                        Some(pos) => ip = pos,
                        None => break, // unmatched bracket → terminate
                    }
                }
            }

            // Loop end: if tape[head0] != 0, jump back to matching '['
            b']' => {
                if tape[head0] != 0 {
                    match find_matching_open(tape, ip) {
                        Some(pos) => ip = pos,
                        None => break, // unmatched bracket → terminate
                    }
                }
            }

            // All other byte values are no-ops
            _ => {}
        }

        ip += 1;
        steps += 1;
    }

    steps
}

/// Find the matching ']' for a '[' at position `pos`.
/// Handles nesting. Returns None if unmatched.
fn find_matching_close(tape: &[u8], pos: usize) -> Option<usize> {
    let mut depth: usize = 1;
    let mut i = pos + 1;
    while i < tape.len() {
        match tape[i] {
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None // unmatched
}

/// Find the matching '[' for a ']' at position `pos`.
/// Handles nesting. Returns None if unmatched.
fn find_matching_open(tape: &[u8], pos: usize) -> Option<usize> {
    if pos == 0 {
        return None;
    }
    let mut depth: usize = 1;
    let mut i = pos - 1;
    loop {
        match tape[i] {
            b']' => depth += 1,
            b'[' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        if i == 0 {
            break;
        }
        i -= 1;
    }
    None // unmatched
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noop_execution() {
        // A tape of all non-instruction bytes should do nothing
        let mut tape = vec![0xFFu8; 64];
        let original = tape.clone();
        execute(&mut tape, 8192);
        assert_eq!(tape, original);
    }

    #[test]
    fn test_increment() {
        // Bytes 0,1 are head positions, execution starts at byte 2.
        // Set head0=2 (pointing at the '+' instruction itself), head1=0.
        // '+' increments tape[head0=2]: b'+' (43) → 44
        let mut tape = vec![2, 0, b'+', 0, 0, 0];
        execute(&mut tape, 10);
        assert_eq!(tape[2], 44);
    }

    #[test]
    fn test_copy_dot() {
        // head0 = tape[0] % len = 3 % 8 = 3, head1 = tape[1] % len = 4 % 8 = 4
        // ip starts at 2
        // tape[2] = '.': tape[head1=4] = tape[head0=3] → tape[4] = tape[3] = 0x42
        let mut tape = vec![3, 4, b'.', 0x42, 0, 0, 0, 0];
        execute(&mut tape, 10);
        assert_eq!(tape[4], 0x42);
    }

    #[test]
    fn test_simple_replicator() {
        // Test that a program in the first half can write BFF instructions
        // into the second half when head1 is initialized to point there.
        // Bytes 0,1 = head positions. Set head0=2 (start of program),
        // head1=64 (start of second half).
        let tape_len = 64;
        let mut tape = vec![0u8; tape_len * 2];
        tape[0] = 2;   // head0 starts at byte 2 (first instruction)
        tape[1] = 64;  // head1 starts at byte 64 (second half)
        // Simple copy loop at bytes 2..: [.>}]
        // Copies tape[head0] to tape[head1], advances both, loops while tape[head0] != 0
        // Place some non-zero data to copy
        tape[2] = b'[';
        tape[3] = b'.';
        tape[4] = b'>';
        tape[5] = b'}';
        tape[6] = b']';
        // Put recognizable data after the loop for head0 to read
        tape[7] = b'+';
        tape[8] = b'-';
        execute(&mut tape, 8192);
        // The copy loop should have written at least some bytes into the right half
        let right_half = &tape[tape_len..];
        let bff_count = right_half.iter().filter(|&&b| {
            matches!(b, b'<' | b'>' | b'{' | b'}' | b'+' | b'-' | b'.' | b',' | b'[' | b']')
        }).count();
        assert!(bff_count > 0, "Program should have copied instructions to right half");
    }
}
