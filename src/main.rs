#![allow(dead_code)]
#![allow(unused_imports)]

use std::fs::File;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Instant;

use anyhow::{Context as _, Result};

use clap::{ArgAction, Parser};

use log::{LevelFilter, info};

use memflow::prelude::v1::*;

use simplelog::*;

use output::Output;

mod analysis;
mod memory;
mod output;
mod source2;

#[derive(Debug, Parser)]
#[command(author, version)]
struct Args {
    /// The name of the memflow connector to use.
    #[arg(short, long)]
    connector: Option<String>,

    /// Additional arguments to pass to the memflow connector.
    #[arg(short = 'a', long)]
    connector_args: Option<String>,

    /// The types of files to generate.
    #[arg(
        short,
        long,
        value_delimiter = ',',
        default_values = ["cs", "hpp", "json", "rs", "zig"]
    )]
    file_types: Vec<String>,

    /// The number of spaces to use per indentation level.
    #[arg(short, long, default_value_t = 4)]
    indent_size: usize,

    /// The output directory to write the generated files to.
    #[arg(short, long, default_value = "output")]
    output: PathBuf,

    /// The name of the game process.
    #[arg(short, long, default_value = "cs2.exe")]
    process_name: String,

    /// Increase logging verbosity. Can be specified multiple times.
    #[arg(short, long, action = ArgAction::Count)]
    verbose: u8,

    /// Prevent creation of the cs2-dumper.log file.
    #[arg(short, long)]
    no_log_file: bool,

    /// Launch the ESP overlay after dumping completes.
    #[cfg(windows)]
    #[arg(long)]
    overlay: bool,
}

fn main() -> Result<()> {
    // Double-clicking the exe (or starting it from a shortcut) passes no arguments: that is
    // UnderBoss for players. Any argument selects the original dumper command line.
    #[cfg(windows)]
    if std::env::args_os().len() <= 1 {
        if let Err(e) = run_underboss() {
            eprintln!("\n[!] {:#}", e);
            cs2_overlay::wait_for_enter();
            std::process::exit(1);
        }

        return Ok(());
    }

    run(Args::parse())
}

/// Waits for CS2, reads fresh offsets from it in memory (nothing is written to disk), then runs
/// the overlay until the player or CS2 closes it.
#[cfg(windows)]
fn run_underboss() -> Result<()> {
    cs2_overlay::print_banner();
    cs2_overlay::wait_for_game();

    let mut os = memflow_native::create_os(&OsArgs::default(), LibArc::default())?;
    let mut process = os.process_by_name("cs2.exe")?;

    // CS2 registers its classes a few seconds after the process starts, so a dump taken too
    // early is incomplete. Retry until every offset the overlay needs is there.
    println!("[*] Reading offsets from CS2...");

    let mut attempts = 0;

    let offsets = loop {
        let parsed = analysis::analyze_all(&mut process)
            .and_then(|result| Output::overlay_json(&result))
            .and_then(|(offsets, client)| cs2_overlay::Offsets::from_json(&offsets, &client));

        match parsed {
            Ok(offsets) => break offsets,
            Err(_) if attempts < 20 => {
                if attempts == 0 {
                    println!("[*] CS2 is still loading, waiting...");
                }

                attempts += 1;

                std::thread::sleep(std::time::Duration::from_secs(3));
            }
            Err(e) => {
                return Err(e.context(
                    "couldn't read offsets from CS2. If CS2 just updated, you need a newer UnderBoss",
                ));
            }
        }
    };

    println!("[+] Offsets ready.");

    cs2_overlay::start(offsets);

    Ok(())
}

fn run(args: Args) -> Result<()> {
    let level_filter = match args.verbose {
        0 => LevelFilter::Error,
        1 => LevelFilter::Warn,
        2 => LevelFilter::Info,
        3 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };

    let mut loggers: Vec<Box<dyn SharedLogger>> = vec![TermLogger::new(
        level_filter,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )];

    // Create the log file by default.
    if !args.no_log_file {
        loggers.push(WriteLogger::new(
            LevelFilter::Info,
            Config::default(),
            File::create("cs2-dumper.log")?,
        ));
    }

    CombinedLogger::init(loggers)?;

    let conn_args = args
        .connector_args
        .map(|s| ConnectorArgs::from_str(&s).expect("unable to parse connector arguments"))
        .unwrap_or_default();

    let mut os = match args.connector {
        Some(conn) => {
            let mut inventory = Inventory::scan();

            inventory
                .builder()
                .connector(&conn)
                .args(conn_args)
                .os("win32")
                .build()?
        }
        None => {
            #[cfg(windows)]
            {
                memflow_native::create_os(&OsArgs::default(), LibArc::default())?
            }
            #[cfg(not(windows))]
            {
                panic!("no connector specified")
            }
        }
    };

    let mut process = os
        .process_by_name(&args.process_name)
        .with_context(|| format!("{} is not running. Start the game first.", args.process_name))?;

    #[cfg(windows)]
    if args.overlay {
        println!("[*] Dumping offsets from {}...", args.process_name);
    }

    let now = Instant::now();

    let result = analysis::analyze_all(&mut process)?;
    let output = Output::new(&args.file_types, args.indent_size, &args.output, &result)?;

    output.dump_all(&mut process)?;

    info!("analysis completed in {:.2?}", now.elapsed());

    #[cfg(windows)]
    if args.overlay {
        let (offsets, client) = Output::overlay_json(&result)?;

        println!("\n[*] Launching overlay...");
        cs2_overlay::start(cs2_overlay::Offsets::from_json(&offsets, &client)?);
    }

    Ok(())
}
