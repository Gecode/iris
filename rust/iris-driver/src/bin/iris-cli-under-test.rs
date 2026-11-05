use std::process::ExitCode;

fn main() -> ExitCode {
    ExitCode::from(gecode_iris_cli::run(std::env::args_os()))
}
