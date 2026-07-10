fn main() {
    match jtv::cli::run() {
        Ok(status) if status != 0 => std::process::exit(status),
        Ok(_) => {}
        Err(jtv::Error::Cancelled) => std::process::exit(130),
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(error.exit_code());
        }
    }
}
