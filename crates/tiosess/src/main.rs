use lioipc::{socket_path, Request};
use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;
use tiohal::input::{device_paths, ComboTracker, InputDevice, Poller};
use tiosess::{dispatch, SessionConfig, SessionManager};

enum DaemonEvent {
    Conn(UnixStream),
    Switch(u32),
}

fn main() {
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| String::from("/etc/afflictio/session.toml"));
    if let Err(e) = run(&config_path) {
        eprintln!("tiosessd: {e}");
        std::process::exit(1);
    }
}

fn run(config_path: &str) -> std::io::Result<()> {
    let raw = fs::read_to_string(config_path).unwrap_or_default();
    let config = SessionConfig::parse_config(&raw).map_err(std::io::Error::other)?;
    let socket = config.socket.clone().unwrap_or_else(socket_path);
    if let Some(dir) = Path::new(&socket).parent() {
        fs::create_dir_all(dir)?;
    }
    let _ = fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket)?;
    let mut manager = SessionManager::new(config);
    if let Err(e) = manager.autostart() {
        eprintln!("tiosessd: autostart skipped: {e}");
    }
    let (tx, rx) = mpsc::channel();
    {
        let tx = tx.clone();
        thread::spawn(move || accept_loop(listener, tx));
    }
    {
        let tx = tx.clone();
        let counter = manager.keyboard_counter();
        thread::spawn(move || keyboard_loop(tx, counter));
    }
    for event in rx {
        match event {
            DaemonEvent::Conn(stream) => {
                if let Err(e) = handle_connection(stream, &mut manager) {
                    eprintln!("tiosessd: request failed: {e}");
                }
            }
            DaemonEvent::Switch(vt) => {
                if let Err(e) = manager.switch_vt(vt) {
                    eprintln!("tiosessd: switch to vt{vt} failed: {e}");
                }
            }
        }
    }
    Ok(())
}

fn accept_loop(listener: UnixListener, tx: mpsc::Sender<DaemonEvent>) {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if tx.send(DaemonEvent::Conn(stream)).is_err() {
                    return;
                }
            }
            Err(e) => eprintln!("tiosessd: accept failed: {e}"),
        }
    }
}

fn keyboard_loop(
    tx: mpsc::Sender<DaemonEvent>,
    counter: std::sync::Arc<std::sync::atomic::AtomicU32>,
) {
    let mut tracker = ComboTracker::new();
    let mut poller = match Poller::new() {
        Ok(poller) => poller,
        Err(e) => {
            eprintln!("tiosessd: input poller unavailable: {e}");
            return;
        }
    };
    let mut known: HashSet<String> = HashSet::new();
    loop {
        for path in device_paths().unwrap_or_default() {
            if known.contains(&path) {
                continue;
            }
            let Ok(device) = InputDevice::open(&path) else {
                continue;
            };
            if poller.add(device).is_ok() {
                known.insert(path);
                counter.store(poller.device_count() as u32, Ordering::Relaxed);
            }
        }
        let result = poller.wait(1000);
        if let Err(e) = &result {
            eprintln!("tiosessd: input poll error: {e}");
        }
        if let Ok(batches) = result {
            for (_, events) in batches {
                for event in events {
                    if let Some(vt) = tracker.feed(&event.kind) {
                        if tx.send(DaemonEvent::Switch(vt as u32)).is_err() {
                            return;
                        }
                    }
                }
            }
        }
    }
}

fn handle_connection(stream: UnixStream, manager: &mut SessionManager) -> std::io::Result<()> {
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
    let req: Request = lioipc::decode(&line)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    let resp = dispatch(manager, req);
    writer.write_all(&lioipc::encode(&resp))?;
    writer.flush()
}
