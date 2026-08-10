use std::process::ExitCode;

fn main() -> ExitCode {
    okft::run(std::env::args_os())
}
