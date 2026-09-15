//! Inspect command boundaries without sending anything to a printer.
use std::{env, fs, process::ExitCode};
use zpl::parse::{Element, ParseContext};

fn main() -> ExitCode {
    let files: Vec<_> = env::args_os().skip(1).collect();
    if files.is_empty() {
        eprintln!("usage: zpl-parse FILE [FILE ...]");
        return ExitCode::FAILURE;
    }
    let mut failed = false;
    for file in files {
        let input = match fs::read(&file) {
            Ok(input) => input,
            Err(error) => {
                eprintln!("{}: {error}", file.to_string_lossy());
                failed = true;
                continue;
            }
        };
        let mut parser = ParseContext::from_bytes(&input);
        let mut count = 0;
        let mut valid = true;
        for result in parser.by_ref() {
            match result {
                Ok(Element::BeforeFirstCommand(_)) => {}
                Ok(_) => count += 1,
                Err(error) => {
                    eprintln!("{}: {error}", file.to_string_lossy());
                    failed = true;
                    valid = false;
                    break;
                }
            }
        }
        if valid {
            println!(
                "{}: {count} commands, {} bytes",
                file.to_string_lossy(),
                input.len()
            );
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
