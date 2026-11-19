use liobinder::{probe, servicemanager, BinderDevice, BinderError};

fn main() {
    let cmd = std::env::args()
        .nth(1)
        .unwrap_or_else(|| String::from("probe"));
    match cmd.as_str() {
        "probe" => match probe() {
            Ok(outcome) => {
                println!(
                    "binderctl: device {} protocol_version {}",
                    outcome.path, outcome.protocol_version
                );
            }
            Err(e) => {
                eprintln!("binderctl: {e}");
                std::process::exit(1);
            }
        },
        "service" => {
            let name = std::env::args().nth(2).unwrap_or_default();
            if name.is_empty() {
                eprintln!("binderctl: usage: binderctl service <name>");
                std::process::exit(1);
            }
            let device = match BinderDevice::open() {
                Ok(device) => device,
                Err(e) => {
                    eprintln!("binderctl: {e}");
                    std::process::exit(1);
                }
            };
            match servicemanager::get_service(&device, &name) {
                Ok(Some(handle)) => println!("binderctl: service {name} handle {handle}"),
                Ok(None) => {
                    println!("binderctl: service {name} not found");
                    std::process::exit(2);
                }
                Err(BinderError::FailedReply) => {
                    println!("binderctl: service {name} not found");
                    std::process::exit(2);
                }
                Err(e) => {
                    eprintln!("binderctl: {e}");
                    std::process::exit(1);
                }
            }
        }
        other => {
            eprintln!("binderctl: unknown command {other}, use probe or service");
            std::process::exit(1);
        }
    }
}
