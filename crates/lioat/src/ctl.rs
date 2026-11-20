use crate::at::AtResponse;
use crate::cfg::DEFAULT_SOCKET;
use crate::modem::{rssi_dbm, ModemState};
use lioipc::{decode, encode, trim_line};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use std::os::unix::net::UnixStream;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModemRequest {
    Status,
    Send { command: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModemResponse {
    Status {
        device: Option<String>,
        manufacturer: Option<String>,
        model: Option<String>,
        pin: String,
        rssi: Option<u8>,
        rssi_dbm: Option<i32>,
        registration: String,
    },
    Lines {
        lines: Vec<String>,
    },
    Error {
        message: String,
    },
}

pub fn socket_path() -> String {
    std::env::var("AFFLICTIO_MODEM_SOCKET").unwrap_or_else(|_| DEFAULT_SOCKET.to_string())
}

pub fn status_response(state: &ModemState) -> ModemResponse {
    ModemResponse::Status {
        device: state.device.clone(),
        manufacturer: state.manufacturer.clone(),
        model: state.model.clone(),
        pin: state.pin.as_str().to_string(),
        rssi: state.rssi,
        rssi_dbm: state.rssi.map(rssi_dbm),
        registration: state.registration.as_str().to_string(),
    }
}

pub fn send_response(result: io::Result<AtResponse>) -> ModemResponse {
    match result {
        Ok(response) => ModemResponse::Lines {
            lines: response.lines,
        },
        Err(e) => ModemResponse::Error {
            message: e.to_string(),
        },
    }
}

pub fn request(req: &ModemRequest) -> io::Result<ModemResponse> {
    let mut stream = UnixStream::connect(socket_path())?;
    stream.write_all(&encode(req))?;
    stream.flush()?;
    let mut reader = io::BufReader::new(&stream);
    let mut line = Vec::new();
    let n = reader.read_until(b'\n', &mut line)?;
    if n == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "lioatd closed the connection",
        ));
    }
    trim_line(&mut line);
    decode(&line).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modem::{PinState, Registration};

    #[test]
    fn status_request_roundtrip() {
        let req = ModemRequest::Status;
        let back: ModemRequest = decode(&encode(&req)).unwrap();
        assert!(matches!(back, ModemRequest::Status));
    }

    #[test]
    fn send_request_roundtrip() {
        let req = ModemRequest::Send {
            command: String::from("AT+CSQ"),
        };
        let back: ModemRequest = decode(&encode(&req)).unwrap();
        match back {
            ModemRequest::Send { command } => assert_eq!(command, "AT+CSQ"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn error_response_roundtrip() {
        let resp = ModemResponse::Error {
            message: String::from("modem not connected"),
        };
        let back: ModemResponse = decode(&encode(&resp)).unwrap();
        assert!(
            matches!(back, ModemResponse::Error { message } if message == "modem not connected")
        );
    }

    #[test]
    fn status_response_carries_state() {
        let state = ModemState {
            device: Some(String::from("/dev/ttyAMA1")),
            manufacturer: Some(String::from("Quectel")),
            model: Some(String::from("EG25-G")),
            pin: PinState::Ready,
            rssi: Some(21),
            registration: Registration::RegisteredHome,
        };
        let resp = status_response(&state);
        let back: ModemResponse = decode(&encode(&resp)).unwrap();
        match back {
            ModemResponse::Status {
                device,
                manufacturer,
                model,
                pin,
                rssi,
                rssi_dbm,
                registration,
            } => {
                assert_eq!(device.as_deref(), Some("/dev/ttyAMA1"));
                assert_eq!(manufacturer.as_deref(), Some("Quectel"));
                assert_eq!(model.as_deref(), Some("EG25-G"));
                assert_eq!(pin, "READY");
                assert_eq!(rssi, Some(21));
                assert_eq!(rssi_dbm, Some(-71));
                assert_eq!(registration, "home");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn send_response_maps_lines_and_errors() {
        let ok = send_response(Ok(AtResponse {
            lines: vec![String::from("+CSQ: 21,99")],
            status: crate::at::AtStatus::Ok,
        }));
        assert!(matches!(ok, ModemResponse::Lines { lines } if lines == ["+CSQ: 21,99"]));
        let err = send_response(Err(io::Error::other("timeout")));
        assert!(matches!(err, ModemResponse::Error { .. }));
    }
}
