use lioat::at::AtChannel;
use lioat::cfg::ModemConfig;
use lioat::ctl::{self, ModemRequest, ModemResponse};
use lioat::modem::{self, ModemState};
use std::fs;
use std::io::{BufRead, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn main() {
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| String::from("/etc/afflictio/lioat.toml"));
    if let Err(e) = run(&config_path) {
        eprintln!("lioatd: {e}");
        std::process::exit(1);
    }
}

fn run(config_path: &str) -> std::io::Result<()> {
    let raw = fs::read_to_string(config_path).unwrap_or_default();
    let config = ModemConfig::parse(&raw).map_err(std::io::Error::other)?;
    let socket = config.socket();
    if let Some(dir) = Path::new(&socket).parent() {
        fs::create_dir_all(dir)?;
    }
    let _ = fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket)?;
    let state = Arc::new(Mutex::new(ModemState::default()));
    let slot = Arc::new(Mutex::new(None::<AtChannel>));
    let response_timeout = config.response_timeout();
    {
        let state = state.clone();
        let slot = slot.clone();
        thread::spawn(move || probe_loop(config, state, slot));
    }
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(e) = handle_connection(stream, &state, &slot, response_timeout) {
                    eprintln!("lioatd: request failed: {e}");
                }
            }
            Err(e) => eprintln!("lioatd: accept failed: {e}"),
        }
    }
    Ok(())
}

fn probe_loop(
    config: ModemConfig,
    state: Arc<Mutex<ModemState>>,
    slot: Arc<Mutex<Option<AtChannel>>>,
) {
    let mut announced = false;
    let mut device: Option<String> = None;
    loop {
        if device.is_none() {
            match modem::open_first(&config.devices) {
                Ok((channel, path)) => {
                    eprintln!("lioatd: modem device {path}");
                    *slot.lock().expect("modem slot mutex") = Some(channel);
                    device = Some(path);
                }
                Err(e) => {
                    eprintln!("lioatd: waiting for modem device: {e}");
                    thread::sleep(Duration::from_secs(2));
                    continue;
                }
            }
        }
        let probe_result = {
            let mut guard = slot.lock().expect("modem slot mutex");
            let mut state_guard = state.lock().expect("modem state mutex");
            match guard.as_mut() {
                Some(channel) => {
                    modem::probe(channel, &mut state_guard, config.response_timeout()).cloned()
                }
                None => Err(std::io::Error::other("modem not connected")),
            }
        };
        match probe_result {
            Ok(snapshot) => {
                if !announced {
                    eprintln!(
                        "lioatd: SMOKE_MODEM_AT_OK path={} manufacturer={} model={} pin={} rssi={}",
                        device.as_deref().unwrap_or(""),
                        snapshot.manufacturer.as_deref().unwrap_or(""),
                        snapshot.model.as_deref().unwrap_or(""),
                        snapshot.pin.as_str(),
                        snapshot.rssi.unwrap_or(0),
                    );
                    announced = true;
                } else {
                    eprintln!(
                        "lioatd: probe ok model={} rssi={}",
                        snapshot.model.as_deref().unwrap_or(""),
                        snapshot.rssi.unwrap_or(0),
                    );
                }
            }
            Err(e) => {
                eprintln!("lioatd: probe failed: {e}");
                *slot.lock().expect("modem slot mutex") = None;
                device = None;
            }
        }
        thread::sleep(Duration::from_secs(config.probe_interval()));
    }
}

fn handle_connection(
    stream: UnixStream,
    state: &Arc<Mutex<ModemState>>,
    slot: &Arc<Mutex<Option<AtChannel>>>,
    timeout_ms: u64,
) -> std::io::Result<()> {
    let mut writer = stream.try_clone()?;
    let mut reader = std::io::BufReader::new(stream);
    let mut line = Vec::new();
    if reader.read_until(b'\n', &mut line)? == 0 {
        return Ok(());
    }
    lioipc::trim_line(&mut line);
    if line.is_empty() {
        return Ok(());
    }
    let req: ModemRequest = lioipc::decode(&line)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    let response = match req {
        ModemRequest::Status => ctl::status_response(&state.lock().expect("modem state mutex")),
        ModemRequest::Send { command } => {
            let mut guard = slot.lock().expect("modem slot mutex");
            match guard.as_mut() {
                Some(channel) => ctl::send_response(channel.send_command(&command, timeout_ms)),
                None => ModemResponse::Error {
                    message: String::from("modem not connected"),
                },
            }
        }
    };
    writer.write_all(&lioipc::encode(&response))?;
    writer.flush()
}
