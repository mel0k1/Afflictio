use tioctl::{parse, run};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse(&args).and_then(run) {
        Ok(()) => {}
        Err(msg) => {
            eprintln!("tioctl: {msg}");
            std::process::exit(2);
        }
    }
}
