#![forbid(unsafe_code)]

fn main() {
    std::process::exit(volmap::cli::run_from(std::env::args_os()));
}
