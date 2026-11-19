use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use std::os::unix::net::UnixStream;

pub const SOCKET_PATH: &str = "/run/afflictio/sessiond.sock";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    Status,
    Start { user: String, command: Vec<String> },
    Switch { vt: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Ok,
    Status {
        sessions: Vec<SessionInfo>,
        active_vt: u32,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: u64,
    pub user: String,
    pub command: String,
    pub vt: u32,
    pub state: String,
}

#[derive(Debug)]
pub enum DecodeError {
    Utf8(std::str::Utf8Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Utf8(e) => write!(f, "invalid utf8: {e}"),
            Self::Json(e) => write!(f, "invalid json: {e}"),
        }
    }
}

impl std::error::Error for DecodeError {}

pub fn socket_path() -> String {
    std::env::var("AFFLICTIO_SOCKET").unwrap_or_else(|_| SOCKET_PATH.to_string())
}

pub fn encode<T: Serialize>(value: &T) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).expect("serializable value");
    bytes.push(b'\n');
    bytes
}

pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, DecodeError> {
    let text = std::str::from_utf8(bytes).map_err(DecodeError::Utf8)?;
    serde_json::from_str(text).map_err(DecodeError::Json)
}

pub fn trim_line(buf: &mut Vec<u8>) {
    while buf
        .last()
        .is_some_and(|b| *b == b'\n' || *b == b'\r' || *b == b' ')
    {
        buf.pop();
    }
}

pub fn request(req: &Request) -> io::Result<Response> {
    let mut stream = UnixStream::connect(socket_path())?;
    stream.write_all(&encode(req))?;
    stream.flush()?;
    let mut reader = io::BufReader::new(&stream);
    let mut line = Vec::new();
    let n = reader.read_until(b'\n', &mut line)?;
    if n == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "sessiond closed the connection",
        ));
    }
    trim_line(&mut line);
    decode(&line).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_request_roundtrip() {
        let req = Request::Status;
        let back: Request = decode(&encode(&req)).unwrap();
        assert!(matches!(back, Request::Status));
    }

    #[test]
    fn start_request_roundtrip() {
        let req = Request::Start {
            user: String::from("mel0k1"),
            command: vec![String::from("/bin/sh"), String::from("-l")],
        };
        let back: Request = decode(&encode(&req)).unwrap();
        match back {
            Request::Start { user, command } => {
                assert_eq!(user, "mel0k1");
                assert_eq!(command, vec!["/bin/sh", "-l"]);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn switch_request_roundtrip() {
        let req = Request::Switch { vt: 2 };
        let back: Request = decode(&encode(&req)).unwrap();
        assert!(matches!(back, Request::Switch { vt: 2 }));
    }

    #[test]
    fn error_response_roundtrip() {
        let resp = Response::Error {
            message: String::from("no tty"),
        };
        let back: Response = decode(&encode(&resp)).unwrap();
        assert!(matches!(back, Response::Error { message } if message == "no tty"));
    }

    #[test]
    fn status_response_roundtrip() {
        let resp = Response::Status {
            sessions: vec![SessionInfo {
                id: 1,
                user: String::from("root"),
                command: String::from("/bin/login"),
                vt: 1,
                state: String::from("running"),
            }],
            active_vt: 1,
        };
        let back: Response = decode(&encode(&resp)).unwrap();
        match back {
            Response::Status {
                sessions,
                active_vt,
            } => {
                assert_eq!(sessions.len(), 1);
                assert_eq!(active_vt, 1);
                assert_eq!(sessions[0].user, "root");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn rejects_garbage() {
        let err = decode::<Request>(b"not json\n");
        assert!(err.is_err());
    }
}
