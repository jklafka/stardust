use std::io::Write;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use stardust::app::App;
use stardust::game::Game;
use stardust::{best, terminal};

const USAGE: &str = "\
Merge cosmic matter from Dust up to a Universe.

Usage: stardust [--seed N]

Options:
  --seed N    seed the random tile placement
  -h, --help  show this help";

fn parse_seed() -> Result<Option<u64>, String> {
    let mut args = std::env::args().skip(1);
    let mut seed = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--seed" => {
                let value = args.next().ok_or("--seed needs a value")?;
                let parsed = value
                    .parse()
                    .map_err(|_| format!("invalid seed: {value}"))?;
                seed = Some(parsed);
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    Ok(seed)
}

fn main() -> ExitCode {
    if std::env::args()
        .skip(1)
        .any(|arg| arg == "-h" || arg == "--help")
    {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let seed = match parse_seed() {
        Ok(seed) => seed.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos() as u64)
        }),
        Err(message) => {
            eprintln!("{message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if !terminal::is_interactive() {
        eprintln!("Stardust needs an interactive terminal.");
        return ExitCode::FAILURE;
    }

    let mut app = App::new(Game::new(seed, best::load()));
    match terminal::run(&mut app) {
        Ok(None) => {
            let game = &app.game;
            let _ = writeln!(
                std::io::stdout(),
                "Thanks for playing. Score {}, best {}.",
                game.score,
                game.best
            );
            ExitCode::SUCCESS
        }
        Ok(Some(signal)) => ExitCode::from(128 + signal as u8),
        Err(error) => {
            eprintln!("Could not set up the terminal: {error}");
            ExitCode::FAILURE
        }
    }
}
