//! v12: the V12 JavaScript engine CLI.
//!
//! Usage:
//!   v12 run <file.js>        Execute a JavaScript file
//!   v12 compile <file.js>    Compile to Wasm and print stats
//!   v12 test262              Run the Test262 test suite

use anyhow::Result;
use clap::{Parser, Subcommand};
use v12_runtime::V12Engine;

mod test262;

#[derive(Parser)]
#[command(name = "v12", about = "V12 JavaScript Engine", version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Execute a JavaScript file.
    Run {
        /// Path to the JavaScript file.
        file: String,
        /// Print the final return value.
        #[arg(short, long)]
        print_result: bool,
    },
    /// Compile a JavaScript file to Wasm and show stats (no execution).
    Compile {
        /// Path to the JavaScript file.
        file: String,
        /// Output path for the .wasm file.
        #[arg(short, long)]
        output: Option<String>,
        /// Print Wasm text format (WAT) to stdout.
        #[arg(short, long)]
        wat: bool,
    },
    /// Run Test262 test suite.
    Test262 {
        /// Path to the test262 directory.
        #[arg(short, long, default_value = "/home/nuun/Desktop/V12/test262")]
        dir: String,
        /// Test suite to run: language, built-ins, annexB, intl402, staging, all
        #[arg(short, long, default_value = "language")]
        suite: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    match cli.command {
        Command::Run { file, print_result } => {
            let engine = V12Engine::new()?;
            let result = engine.run_file(&file)?;
            if print_result {
                println!("=> {}", result);
            }
        }

        Command::Compile { file, output, wat } => {
            let engine = V12Engine::new()?;
            let (bytes, func_count) = engine.compile_file(&file)?;
            println!("Compiled {} → {} bytes of Wasm ({} functions)", file, bytes.len(), func_count);
            if wat {
                match wasmprinter::print_bytes(&bytes) {
                    Ok(wat_text) => println!("{}", wat_text),
                    Err(e) => println!("Error printing WAT: {}", e),
                }
            }
            if let Some(out) = output {
                std::fs::write(&out, &bytes)?;
                println!("Written to {}", out);
            }
        }

        Command::Test262 { dir, suite } => {
            test262::run_test262(&dir, &suite)?;
        }
    }

    Ok(())
}
