use std::process;

fn main() {
    let path = afflictio_init::default_config_path();
    if let Err(e) = afflictio_init::run(&path) {
        eprintln!("afflictio-init: {e}");
        process::exit(1);
    }
}
