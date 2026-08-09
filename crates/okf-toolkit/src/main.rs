use std::process::ExitCode;

fn main() -> ExitCode {
    okf_toolkit::run(std::env::args_os())
}
