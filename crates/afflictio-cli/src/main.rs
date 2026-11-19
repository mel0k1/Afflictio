use afflictio_cli::{parse, run};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse(&args).and_then(run) {
        Ok(()) => {}
        Err(msg) => {
            eprintln!("afflictioctl: {msg}");
            std::process::exit(2);
        }
    }
}
