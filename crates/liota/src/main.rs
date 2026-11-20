use liota::cfg::SlotsConfig;
use liota::ops;
use liota::payload;
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
        Some("verify") => {
            let payload = args
                .get(1)
                .ok_or_else(|| String::from("verify needs a payload path"))?;
            Ok(Command::Verify(payload.clone()))
        }
        Some("keygen") => {
            let key = args
                .get(1)
                .ok_or_else(|| String::from("keygen needs a private key path"))?;
            let pub_key = args
                .get(2)
                .ok_or_else(|| String::from("keygen needs a public key path"))?;
            Ok(Command::Keygen(key.clone(), pub_key.clone()))
        }
        Some("sign") => {
            let key = args
                .get(1)
                .ok_or_else(|| String::from("sign needs a private key path"))?;
            let image = args
                .get(2)
                .ok_or_else(|| String::from("sign needs an image path"))?;
            let out = args
                .get(3)
                .ok_or_else(|| String::from("sign needs an output path"))?;
            let version = args.get(4).cloned();
            Ok(Command::Sign(key.clone(), image.clone(), out.clone(), version))
        }
        _ => Err(String::from(
            "usage: liota status | liota switch | liota verify PAYLOAD | liota install PAYLOAD | liota keygen KEY PUB | liota sign KEY IMAGE UPDATE [VERSION]",
        )),
    }
}

enum Command {
    Status,
    Switch,
    Install(String),
    Verify(String),
    Keygen(String, String),
    Sign(String, String, String, Option<String>),
}

fn update_pub_path() -> String {
    std::env::var("AFFLICTIO_UPDATE_PUB")
        .unwrap_or_else(|_| String::from("/etc/afflictio/updates.pub"))
}

fn load_config() -> Result<SlotsConfig, String> {
    let config_path =
        std::env::var("AFFLICTIO_SLOTS").unwrap_or_else(|_| liota::cfg::DEFAULT_CONFIG.to_string());
    let raw = fs::read_to_string(&config_path).unwrap_or_default();
    SlotsConfig::parse(&raw).map_err(|e| format!("slots config {config_path}: {e}"))
}

fn run(command: Command) -> Result<(), String> {
    match command {
        Command::Status => {
            let config = load_config()?;
            let cmdline = fs::read_to_string("/proc/cmdline").unwrap_or_default();
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
            let config = load_config()?;
            let cmdline = fs::read_to_string("/proc/cmdline").unwrap_or_default();
            let target = ops::switch(&config, &cmdline).map_err(|e| e.to_string())?;
            println!("liota: boot slot switched to {target}");
            Ok(())
        }
        Command::Verify(payload) => {
            let manifest =
                payload::verify(&payload, &update_pub_path()).map_err(|e| e.to_string())?;
            println!(
                "liota: payload verified version={} size={} sha256={}",
                manifest.version, manifest.size, manifest.sha256
            );
            Ok(())
        }
        Command::Install(payload) => {
            let config = load_config()?;
            let cmdline = fs::read_to_string("/proc/cmdline").unwrap_or_default();
            let target = inactive_slot(&config, &cmdline)?;
            let partition = config.partition_path(target.partition);
            let (manifest, written) =
                payload::install_image(&payload, &update_pub_path(), &partition)
                    .map_err(|e| e.to_string())?;
            println!(
                "liota: verified version={}, wrote {written} bytes to slot {} ({partition})",
                manifest.version, target.name
            );
            Ok(())
        }
        Command::Keygen(key, pub_key) => {
            payload::keygen(&key, &pub_key).map_err(|e| e.to_string())?;
            println!("liota: keypair written to {key} and {pub_key}");
            Ok(())
        }
        Command::Sign(key, image, out, version) => {
            let version = version.unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
            let manifest =
                payload::build(&image, &version, &key, &out).map_err(|e| e.to_string())?;
            println!(
                "liota: signed {} (version={}, size={}, sha256={})",
                out, manifest.version, manifest.size, manifest.sha256
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
