use afflictio_ipc::{decode, encode, socket_path, Request};
use std::fs;
use std::io::{BufRead, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

use afflictio_session::{dispatch, SessionConfig, SessionManager};

fn main() {
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| String::from("/etc/afflictio/session.toml"));
    if let Err(e) = run(&config_path) {
        eprintln!("afflictio-sessiond: {e}");
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
        eprintln!("afflictio-sessiond: autostart skipped: {e}");
    }
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(e) = handle_connection(stream, &mut manager) {
                    eprintln!("afflictio-sessiond: request failed: {e}");
                }
            }
            Err(e) => eprintln!("afflictio-sessiond: accept failed: {e}"),
        }
    }
    Ok(())
}

fn handle_connection(stream: UnixStream, manager: &mut SessionManager) -> std::io::Result<()> {
    let mut writer = stream.try_clone()?;
    let mut reader = std::io::BufReader::new(stream);
    let mut line = Vec::new();
    if reader.read_until(b'\n', &mut line)? == 0 {
        return Ok(());
    }
    afflictio_ipc::trim_line(&mut line);
    if line.is_empty() {
        return Ok(());
    }
    let req: Request = decode(&line)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    let resp = dispatch(manager, req);
    writer.write_all(&encode(&resp))?;
    writer.flush()
}
