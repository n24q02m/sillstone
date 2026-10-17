//! `gen-reference`: regenerates or verifies `reference.json`.
//!
//! `gen-reference` recomputes every committed vector with the library
//! pipeline and either rewrites the file (default) or checks the
//! committed copy byte-for-byte (`verify`, the CI gate).

use std::process::ExitCode;

fn main() -> ExitCode {
    run(std::env::args().nth(1).as_deref())
}

fn run(arg: Option<&str>) -> ExitCode {
    match arg {
        Some("verify") => match sillstone_image::reference::verify() {
            Ok(()) => {
                println!("reference vectors are current");
                ExitCode::SUCCESS
            }
            Err(why) => {
                eprintln!("FAIL: {why}");
                ExitCode::FAILURE
            }
        },
        Some("gen") | Some("generate") | None => {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("reference.json");
            std::fs::write(&path, sillstone_image::reference::reference_json())
                .unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
            println!("wrote {}", path.display());
            ExitCode::SUCCESS
        }
        Some(_) => {
            eprintln!("usage: gen-reference [verify]");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::run;
    use std::process::ExitCode;

    /// The verify mode confirms the committed file from a checkout.
    #[test]
    fn verify_mode_reports_current() {
        assert_eq!(run(Some("verify")), ExitCode::SUCCESS);
    }

    /// Unknown arguments are a usage error with exit code 2.
    #[test]
    fn unknown_argument_is_usage_error() {
        assert_eq!(run(Some("bogus")), ExitCode::from(2));
    }
}
