fn main() {
    if let Err(error) = ssot_manager::run() {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
}
