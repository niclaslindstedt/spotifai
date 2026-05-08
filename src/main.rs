use std::process::ExitCode;

fn main() -> ExitCode {
    match spotifai::cli::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            spotifai::output::error(&format!("{e:#}"));
            ExitCode::FAILURE
        }
    }
}
