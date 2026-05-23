//! SeekerSim process entry point.

mod cli;

use std::process;

fn main() {
    if let Err(code) = cli::run() {
        process::exit(code);
    }
}
