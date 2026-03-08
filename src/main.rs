// =============================================================================
// bff-replicator: A Rust implementation of the BFF primordial soup experiment
// =============================================================================
//
// Based on: "Computational Life: How Well-formed, Self-replicating Programs
// Emerge from Simple Interaction" — Agüera y Arcas et al. (2024)
// Paper: https://arxiv.org/abs/2406.19108
//
// This implements the "primordial soup" simulation where:
//   - A large pool of fixed-length byte strings ("tapes") is initialized randomly.
//   - Each tape is a program in the BFF instruction set (extended Brainfuck).
//   - Pairs of tapes are concatenated, executed, then split back — modifying both.
//   - No fitness function exists. No selection pressure is applied.
//   - Self-replicating programs emerge spontaneously from this process.
//
// The key insight: computation itself is a dynamical attractor. In substrates
// that support computation, self-replicators arise because replication is a
// kinetically stable state — once a replicator appears, it tends to persist
// and spread, whereas non-replicators are continually overwritten.
//
// ARCHITECTURE OVERVIEW
// ---------------------
// 1. BFF Interpreter (mod bff):
//    - 10 instructions operating on a single tape with 3 pointers
//    - instruction pointer (ip), head0 (read/primary), head1 (write/secondary)
//    - data and instructions share the same tape (von Neumann architecture)
//
// 2. Primordial Soup (mod soup):
//    - Pool of N tapes, each of length L bytes
//    - Each epoch: randomly pair tapes, concatenate, execute, split
//    - Optional background mutation at configurable rate
//
// 3. Complexity Metrics (mod metrics):
//    - Shannon entropy of byte distribution
//    - Unique token count (proxy for diversity)
//    - High-order entropy approximation (Shannon entropy minus compression ratio)
//    - State transition detection
//
// 4. Spatial Mode (mod spatial):
//    - 2D grid arrangement where tapes only interact with neighbors
//    - Enables visualization of replicator wavefronts
//
// USAGE
// -----
//   cargo run --release -- --help
//   cargo run --release -- --soup-size 4096 --tape-len 64 --epochs 16000
//   cargo run --release -- --mode spatial --width 120 --height 68

use std::time::Instant;

mod bff;
mod metrics;
mod soup;
mod spatial;

use clap::Parser;

/// BFF Primordial Soup: Spontaneous emergence of self-replicating programs
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Simulation mode: "soup" (well-mixed) or "spatial" (2D grid)
    #[arg(long, default_value = "soup")]
    mode: String,

    /// Number of tapes in the soup (soup mode)
    #[arg(long, default_value_t = 4096)]
    soup_size: usize,

    /// Length of each tape in bytes
    #[arg(long, default_value_t = 64)]
    tape_len: usize,

    /// Number of epochs to simulate
    #[arg(long, default_value_t = 16000)]
    epochs: usize,

    /// Background mutation rate (probability per byte per epoch)
    /// Paper default: 0.00024 (0.024%). Use 0.0 for no mutation.
    #[arg(long, default_value_t = 0.00024)]
    mutation_rate: f64,

    /// Max execution steps per interaction (paper uses 2^13 = 8192)
    #[arg(long, default_value_t = 8192)]
    max_steps: usize,

    /// Grid width (spatial mode)
    #[arg(long, default_value_t = 120)]
    width: usize,

    /// Grid height (spatial mode)
    #[arg(long, default_value_t = 68)]
    height: usize,

    /// Interaction radius (spatial mode, Chebyshev distance)
    #[arg(long, default_value_t = 2)]
    radius: usize,

    /// How often to print statistics (every N epochs)
    #[arg(long, default_value_t = 100)]
    report_interval: usize,

    /// Random seed (0 = use system entropy)
    #[arg(long, default_value_t = 0)]
    seed: u64,

    /// Print a sample tape when a state transition is detected
    #[arg(long, default_value_t = true)]
    show_replicator: bool,

    /// Show hex bytes alongside BFF view when printing tapes
    #[arg(long, default_value_t = false)]
    show_hex: bool,

    /// Use Rayon parallel execution for BFF pair processing
    #[arg(long, default_value_t = false)]
    parallel: bool,
}

fn main() {
    let args = Args::parse();

    println!();
    println!("Bff_replicator V1.04");
    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  BFF Primordial Soup — Computational Life Experiment         ║");
    println!("║  After Agüera y Arcas et al. (2024), arXiv:2406.19108        ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    match args.mode.as_str() {
        "soup" => run_soup(&args),
        "spatial" => run_spatial(&args),
        _ => {
            eprintln!("Unknown mode: {}. Use 'soup' or 'spatial'.", args.mode);
            std::process::exit(1);
        }
    }
}

fn run_soup(args: &Args) {

    // If seed to manually set to zero the it will be derived from entropy. In this implementation this will
    // never happen because the code below sets the seed toi be random if it is not set in args.
    // we do this here because if we managed to find a state transtion we want the seed used ti be recorded
    // at run time in order to recretae the simulation run.
    //
    // TODO: we could just move the code here that derived the seed from entropy IFF seed == 0, 
    // or we could set seed to -1 for example to force derivation from entropy as per original code
    //
    let seed = if args.seed == 0 {
        rand::random::<u64>()
    } else {
        args.seed
    };

    println!("Mode:              Well-mixed primordial soup");
    println!("Soup size:         {} tapes", args.soup_size);
    println!("Tape length:       {} bytes", args.tape_len);
    println!("Epochs:            {}", args.epochs);
    println!("Mutation rate:     {:.6} ({:.4}%)", args.mutation_rate, args.mutation_rate * 100.0);
    println!("Max steps:         {}", args.max_steps);
    println!("Show replicator:   {}", if args.show_replicator == true { "yes" } else { "no" });
    println!("Parallel:          {}", if args.parallel {
        format!("yes ({} threads)", rayon::current_num_threads())
    } else {
        "no".to_string()
    });
    println!("Seed:              {}", seed);
    println!();
    println!("{:>8} {:>10} {:>10} {:>10} {:>12} {:>12}  {}",
             "epoch", "entropy", "hi-order", "unique", "top_token%", "uniq_tapes", "status");
    println!("{}", "-".repeat(86));

    let mut sim = soup::Soup::new(
        args.soup_size,
        args.tape_len,
        args.max_steps,
        args.mutation_rate,
        seed,
    );

    let start = Instant::now();
    let mut transitioned = false;
    let mut transition_epoch = 0;

    for epoch in 1..=args.epochs {
        if args.parallel {
            sim.step_parallel();
        } else {
            sim.step();
        }

        if epoch % args.report_interval == 0 || epoch == 1 {
            let stats = sim.compute_stats();
            let status = if stats.high_order_entropy >= 1.0 && !transitioned {
                transitioned = true;
                transition_epoch = epoch;
                " *** STATE TRANSITION ***"
            } else if transitioned {
                " [post-transition]"
            } else {
                ""
            };

            println!(
                "{:>8} {:>10.4} {:>10.4} {:>10} {:>11.2}% {:>12}  {}",
                epoch,
                stats.shannon_entropy,
                stats.high_order_entropy,
                stats.unique_bytes,
                stats.top_token_fraction * 100.0,
                stats.unique_tapes,
                status,
            );

            if transitioned && args.show_replicator && epoch == transition_epoch {
                println!();
                let sample = sim.get_most_common_tape();
                println!("  Most common tape:");
                print_tape(&sample, args.tape_len, args.show_hex);

                if let Some(replicator) = sim.get_most_common_replicator_tape(3) {
                    if replicator != sample {
                        println!("  Most common replicator tape:");
                        print_tape(&replicator, args.tape_len, args.show_hex);
                    }
                }
                println!();
            }
        }
    }

    let elapsed = start.elapsed();
    println!();
    println!("════════════════════════════════════════════════════");
    println!("Simulation complete in {:.2}s", elapsed.as_secs_f64());
    if transitioned {
        println!("State transition detected at epoch {}", transition_epoch);
    } else {
        println!("No state transition detected within {} epochs.", args.epochs);
        println!("(This is expected ~60% of the time with default settings.)");
    }
    println!("════════════════════════════════════════════════════");
}

fn run_spatial(args: &Args) {
    let n = args.width * args.height;
    println!("Mode:          2D spatial grid");
    println!("Grid:          {} x {} = {} tapes", args.width, args.height, n);
    println!("Tape length:   {} bytes", args.tape_len);
    println!("Radius:        {}", args.radius);
    println!("Epochs:        {}", args.epochs);
    println!("Mutation rate: {:.6}", args.mutation_rate);
    println!();
    println!("{:>8} {:>10} {:>10} {:>10}  {}",
             "epoch", "entropy", "hi-order", "unique", "status");
    println!("{}", "-".repeat(60));

    let mut sim = spatial::SpatialSoup::new(
        args.width,
        args.height,
        args.tape_len,
        args.max_steps,
        args.mutation_rate,
        args.radius,
        args.seed,
    );

    let start = Instant::now();
    let mut transitioned = false;

    for epoch in 1..=args.epochs {
        sim.step();

        if epoch % args.report_interval == 0 || epoch == 1 {
            let stats = sim.compute_stats();
            let status = if stats.high_order_entropy >= 1.0 && !transitioned {
                transitioned = true;
                " *** STATE TRANSITION ***"
            } else {
                ""
            };

            println!(
                "{:>8} {:>10.4} {:>10.4} {:>10}  {}",
                epoch,
                stats.shannon_entropy,
                stats.high_order_entropy,
                stats.unique_bytes,
                status,
            );
        }
    }

    let elapsed = start.elapsed();
    println!();
    println!("Simulation complete in {:.2}s", elapsed.as_secs_f64());
}

/// Pretty-print a tape, highlighting BFF instruction characters
fn print_tape(tape: &[u8], tape_len: usize, show_hex: bool) {
    let display_len = tape_len.min(tape.len());
    print!("  BFF: [");
    for &b in &tape[..display_len] {
        match b {
            b'<' | b'>' | b'{' | b'}' | b'+' | b'-' | b'.' | b',' | b'[' | b']' => {
                print!("{}", b as char);
            }
            0 => print!("0"),
            _ => print!("·"),
        }
    }
    println!("]");
    if show_hex {
        print!("  HEX: ");
        for &b in &tape[..display_len] {
            print!("{:02x} ", b);
        }
        println!();
    }
}
