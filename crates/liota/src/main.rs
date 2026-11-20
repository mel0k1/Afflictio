use liota::cfg::SlotsConfig;
use liota::ops;
use std::fs;
use std::process;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse(&args).and_then(run) {
        Ok(()) => {}
        Err(msg) => {
            eprintln!("liota: {msg}");
            process::exit(2);
        }
    }
}

fn parse(args: &[String]) -> Result<Command, String> {
    match args.first().map(String::as_str) {
        Some("status") => Ok(Command::Status),
        Some("switch") => Ok(Command::Switch),
        Some("install") => {
            let payload = args
                .get(1)
                .ok_or_else(|| String::from("install needs a payload path"))?;
            Ok(Command::Install(payload.clone()))
        }
        _ => Err(String::from(
            "usage: liota status | liota switch | liota install PAYLOAD",
        )),
    }
}

enum Command {
    Status,
    Switch,
    Install(String),
}

fn run(command: Command) -> Result<(), String> {
    let config_path =
        std::env::var("AFFLICTIO_SLOTS").unwrap_or_else(|_| liota::cfg::DEFAULT_CONFIG.to_string());
    let raw = fs::read_to_string(&config_path).unwrap_or_default();
    let config =
        SlotsConfig::parse(&raw).map_err(|e| format!("slots config {config_path}: {e}"))?;
    let cmdline = fs::read_to_string("/proc/cmdline").unwrap_or_default();
    match command {
        Command::Status => {
            let report = ops::status(&config, &cmdline).map_err(|e| e.to_string())?;
            let doc = serde_json::json!({
                "active": report.active,
                "inactive": report.inactive,
                "boot_ok": report.boot_ok,
            });
            println!("{doc}");
            Ok(())
        }
        Command::Switch => {
            let target = ops::switch(&config, &cmdline).map_err(|e| e.to_string())?;
            println!("liota: boot slot switched to {target}");
            Ok(())
        }
        Command::Install(payload) => {
            let target = inactive_slot(&config, &cmdline)?;
            let partition = config.partition_path(target.partition);
            let written = ops::install(&payload, &partition).map_err(|e| e.to_string())?;
            println!(
                "liota: wrote {written} bytes to slot {} ({partition})",
                target.name
            );
            Ok(())
        }
    }
}

fn inactive_slot(config: &SlotsConfig, cmdline: &str) -> Result<liota::cfg::SlotSpec, String> {
    let uuid = liota::cmdline::active_uuid(cmdline)
        .ok_or_else(|| String::from("cmdline carries no root=UUID="))?;
    let active = config
        .by_uuid(&uuid)
        .ok_or_else(|| format!("active uuid {uuid} is not in the slot list"))?;
    config
        .other(&active.name)
        .cloned()
        .ok_or_else(|| String::from("no second slot to install into"))
}
