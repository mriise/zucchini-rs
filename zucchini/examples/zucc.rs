//! Command line front end for trying Zucchini on real files.
//!
//! Set ZUCCHINI_LOG to error, warning or info to see Zucchini's own messages.

use std::process::ExitCode;
use std::time::Instant;

use zucchini::LogLevel;

const USAGE: &str = "usage: zucc gen [--raw] <old> <new> <patch>
       zucc apply <old> <patch> <new>
       zucc info <patch>";

fn main() -> ExitCode {
    let level = match std::env::var("ZUCCHINI_LOG").as_deref() {
        Ok("error") => LogLevel::Error,
        Ok("warning") => LogLevel::Warning,
        Ok("info") => LogLevel::Info,
        _ => LogLevel::Off,
    };
    zucchini::set_log_level(level);

    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[&str]) -> Result<(), String> {
    match args {
        ["gen", rest @ ..] => {
            let (raw, rest) = match rest {
                ["--raw", rest @ ..] => (true, rest),
                _ => (false, rest),
            };
            let [old_path, new_path, patch_path] = rest else { return Err(USAGE.into()) };
            let (old, new) = (read(old_path)?, read(new_path)?);
            let started = Instant::now();
            let patch = if raw { zucchini::generate_raw(&old, &new) } else { zucchini::generate(&old, &new) }
                .map_err(|error| error.to_string())?;
            write(patch_path, &patch)?;
            println!(
                "{patch_path}: {} bytes, {} to {} bytes, {:.1}s",
                patch.len(),
                old.len(),
                new.len(),
                started.elapsed().as_secs_f64()
            );
            Ok(())
        }
        ["apply", old_path, patch_path, new_path] => {
            let (old, patch) = (read(old_path)?, read(patch_path)?);
            let started = Instant::now();
            let new = zucchini::apply(&old, &patch).map_err(|error| error.to_string())?;
            write(new_path, &new)?;
            println!("{new_path}: {} bytes, {:.1}s", new.len(), started.elapsed().as_secs_f64());
            Ok(())
        }
        ["info", patch_path] => {
            let header = zucchini::read_header(&read(patch_path)?).map_err(|error| error.to_string())?;
            println!("{header:#?}");
            Ok(())
        }
        _ => Err(USAGE.into()),
    }
}

fn read(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|error| format!("{path}: {error}"))
}

fn write(path: &str, data: &[u8]) -> Result<(), String> {
    std::fs::write(path, data).map_err(|error| format!("{path}: {error}"))
}
