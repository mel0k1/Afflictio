use lioat::ctl::{self, ModemRequest, ModemResponse};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse(&args).and_then(execute) {
        Ok(()) => {}
        Err(msg) => {
            eprintln!("modemctl: {msg}");
            std::process::exit(2);
        }
    }
}

fn parse(args: &[String]) -> Result<ModemRequest, String> {
    match args.first().map(String::as_str) {
        Some("status") => Ok(ModemRequest::Status),
        Some("at") => {
            let command = args
                .get(1)
                .ok_or_else(|| String::from("at needs a command argument"))?;
            Ok(ModemRequest::Send {
                command: command.clone(),
            })
        }
        _ => Err(String::from("usage: modemctl status | modemctl at CMD")),
    }
}

fn execute(req: ModemRequest) -> Result<(), String> {
    match ctl::request(&req) {
        Ok(response) => {
            print_response(response);
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

fn print_response(response: ModemResponse) {
    match response {
        ModemResponse::Status {
            device,
            manufacturer,
            model,
            pin,
            rssi,
            rssi_dbm,
            registration,
        } => {
            let doc = serde_json::json!({
                "device": device,
                "manufacturer": manufacturer,
                "model": model,
                "pin": pin,
                "rssi": rssi,
                "rssi_dbm": rssi_dbm,
                "registration": registration,
            });
            println!("{doc}");
        }
        ModemResponse::Lines { lines } => {
            for line in lines {
                println!("{line}");
            }
        }
        ModemResponse::Error { message } => {
            eprintln!("modemctl: {message}");
            std::process::exit(1);
        }
    }
}
