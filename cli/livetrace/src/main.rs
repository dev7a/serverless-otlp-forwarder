//! `livetrace` is a Command Line Interface (CLI) tool for real-time tailing
//! of AWS CloudWatch Logs, specifically designed to capture and forward
//! OTLP (OpenTelemetry Protocol) traces and logs.

// Standard Library
use std::io::stdout;
use std::process;

// External Crates
use anyhow::Result;
use clap::{CommandFactory, Parser}; // Added CommandFactory
use clap_complete::generate; // Added generate

// Workspace Crates (the library part of this crate)
// CliArgs and Commands are defined in src/cli.rs and re-exported from src/lib.rs
// run_livetrace is the main entry point in src/lib.rs
use livetrace::{run_livetrace, CliArgs, Commands}; // Added Commands

/// Main entry point for the `livetrace` binary.
///
/// This function performs the following steps:
/// 1. Parses command-line arguments using `clap`.
/// 2. Calls the `run_livetrace` function from the `livetrace` library crate,
///    passing the parsed arguments. This function contains the core application logic.
/// 3. If `run_livetrace` returns an error, it prints the error to `stderr`
///    and exits the process with a status code of 1.
/// 4. If `run_livetrace` completes successfully, the program exits with a status code of 0.
#[tokio::main]
async fn main() -> Result<()> {
    // Parse command-line arguments.
    // `CliArgs` struct (derived with `clap::Parser`) handles parsing.
    let args = CliArgs::parse();

    if let Some(command) = &args.command {
        // Use a reference to args.command
        match command {
            Commands::GenerateCompletions { shell } => {
                let mut cmd = CliArgs::command();
                let bin_name = cmd.get_name().to_string();
                generate(*shell, &mut cmd, bin_name, &mut stdout()); // Dereference shell
                return Ok(()); // Exit after generating completions
            }
        }
    }

    // Execute the core application logic if no subcommand was handled.
    // Note: run_livetrace now takes ownership of args. If GenerateCompletions was run,
    // args is not used further here, which is fine. If no subcommand, args is moved into run_livetrace.
    if let Err(e) = run_livetrace(args).await {
        // Output any errors to stderr and exit with a non-zero status code.
        // The {:#} format specifier for anyhow::Error provides a detailed error message,
        // often including the chain of causes.
        eprintln!("Error: {:#}", e);
        process::exit(1); // Exit with a non-zero status code to indicate failure
    }

    Ok(()) // Exit successfully
}
