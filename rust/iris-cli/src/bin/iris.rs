use std::process::ExitCode;

fn main() -> ExitCode {
    ExitCode::from(iris_cli::run(std::env::args_os()))
}
