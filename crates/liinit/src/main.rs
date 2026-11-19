use std::process;

fn main() {
    let path = liinit::default_config_path();
    if let Err(e) = liinit::run(&path) {
        eprintln!("liinit: {e}");
        process::exit(1);
    }
}
