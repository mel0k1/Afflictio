use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::time::Duration;

const INPUT_EVENT_SIZE: usize = 24;
const READ_BUF_EVENTS: usize = 32;
const EPOLL_MAX_EVENTS: usize = 16;

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_ABS: u16 = 0x03;
const SYN_REPORT: u16 = 0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    Key { code: u16, value: i32 },
    Abs { code: u16, value: i32 },
    Syn,
    Other { kind: u16, code: u16, value: i32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputEvent {
    pub timestamp: Duration,
    pub kind: EventKind,
}

#[derive(Debug)]
pub struct InputDevice {
    path: String,
    name: String,
    file: File,
}

impl InputDevice {
    pub fn open(path: &str) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).open(path)?;
        let name = device_name(&file)?;
        Ok(Self {
            path: path.to_string(),
            name,
            file,
        })
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn read_events(&mut self) -> io::Result<Vec<InputEvent>> {
        let mut buf = [0u8; INPUT_EVENT_SIZE * READ_BUF_EVENTS];
        let n = unsafe { libc::read(self.file.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::WouldBlock || err.kind() == io::ErrorKind::Interrupted {
                return Ok(Vec::new());
            }
            return Err(err);
        }
        Ok(parse_events(&buf[..n as usize]))
    }
}

pub fn device_paths() -> io::Result<Vec<String>> {
    let mut paths = Vec::new();
    for entry in std::fs::read_dir("/dev/input")? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("event") {
            paths.push(entry.path().to_string_lossy().into_owned());
        }
    }
    paths.sort();
    Ok(paths)
}

fn device_name(file: &File) -> io::Result<String> {
    let mut buf = [0u8; 256];
    let req = eviocg(0x06, buf.len());
    let n = unsafe { libc::ioctl(file.as_raw_fd(), req, buf.as_mut_ptr()) };
    if n < 0 {
        return Ok(String::from("unknown"));
    }
    let end = (n as usize).min(buf.len());
    let text = String::from_utf8_lossy(&buf[..end]);
    Ok(text.trim_end_matches('\0').trim().to_string())
}

fn eviocg(nr: u8, size: usize) -> libc::c_ulong {
    (2u64 << 30) | ((size as u64) << 16) | (0x45u64 << 8) | nr as u64
}

fn le_u64(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes.try_into().expect("slice of exact length"))
}

fn le_u16(bytes: &[u8]) -> u16 {
    u16::from_le_bytes(bytes.try_into().expect("slice of exact length"))
}

fn le_i32(bytes: &[u8]) -> i32 {
    i32::from_le_bytes(bytes.try_into().expect("slice of exact length"))
}

pub fn parse_events(bytes: &[u8]) -> Vec<InputEvent> {
    let mut events = Vec::new();
    let mut offset = 0;
    while offset + INPUT_EVENT_SIZE <= bytes.len() {
        let chunk = &bytes[offset..offset + INPUT_EVENT_SIZE];
        let sec = le_u64(&chunk[0..8]);
        let usec = le_u64(&chunk[8..16]);
        let kind = le_u16(&chunk[16..18]);
        let code = le_u16(&chunk[18..20]);
        let value = le_i32(&chunk[20..24]);
        let kind = match kind {
            EV_SYN if code == SYN_REPORT => EventKind::Syn,
            EV_KEY => EventKind::Key { code, value },
            EV_ABS => EventKind::Abs { code, value },
            _ => EventKind::Other { kind, code, value },
        };
        events.push(InputEvent {
            timestamp: Duration::new(sec, (usec * 1_000) as u32),
            kind,
        });
        offset += INPUT_EVENT_SIZE;
    }
    events
}

pub struct Poller {
    epoll: File,
    devices: Vec<InputDevice>,
}

impl Poller {
    pub fn new() -> io::Result<Self> {
        let fd = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let epoll = unsafe { File::from_raw_fd(fd) };
        Ok(Self {
            epoll,
            devices: Vec::new(),
        })
    }

    pub fn add(&mut self, device: InputDevice) -> io::Result<()> {
        let mut ev = libc::epoll_event {
            events: libc::EPOLLIN as u32,
            u64: device.file.as_raw_fd() as u64,
        };
        let rc = unsafe {
            libc::epoll_ctl(
                self.epoll.as_raw_fd(),
                libc::EPOLL_CTL_ADD,
                device.file.as_raw_fd(),
                &mut ev,
            )
        };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }
        self.devices.push(device);
        Ok(())
    }

    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    pub fn wait(&mut self, timeout_ms: i32) -> io::Result<Vec<(usize, Vec<InputEvent>)>> {
        let mut events = [libc::epoll_event { events: 0, u64: 0 }; EPOLL_MAX_EVENTS];
        let n = unsafe {
            libc::epoll_wait(
                self.epoll.as_raw_fd(),
                events.as_mut_ptr(),
                EPOLL_MAX_EVENTS as libc::c_int,
                timeout_ms,
            )
        };
        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                return Ok(Vec::new());
            }
            return Err(err);
        }
        let mut out = Vec::new();
        for slot in &events[..n as usize] {
            let fd = slot.u64 as i32;
            if let Some(idx) = self.devices.iter().position(|d| d.file.as_raw_fd() == fd) {
                let events = self.devices[idx].read_events()?;
                out.push((idx, events));
            }
        }
        Ok(out)
    }
}

impl Default for Poller {
    fn default() -> Self {
        Self::new().expect("epoll available")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event_bytes(kind: u16, code: u16, value: i32, sec: u64, usec: u64) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(INPUT_EVENT_SIZE);
        bytes.extend_from_slice(&sec.to_le_bytes());
        bytes.extend_from_slice(&usec.to_le_bytes());
        bytes.extend_from_slice(&kind.to_le_bytes());
        bytes.extend_from_slice(&code.to_le_bytes());
        bytes.extend_from_slice(&value.to_le_bytes());
        bytes
    }

    #[test]
    fn parses_key_press() {
        let mut bytes = event_bytes(EV_KEY, 30, 1, 100, 500_000);
        bytes.extend_from_slice(&event_bytes(EV_SYN, SYN_REPORT, 0, 100, 500_001));
        let events = parse_events(&bytes);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, EventKind::Key { code: 30, value: 1 });
        assert_eq!(events[0].timestamp, Duration::new(100, 500_000_000));
    }

    #[test]
    fn parses_absolute_event() {
        let bytes = event_bytes(EV_ABS, 0x35, 1024, 1, 2);
        let events = parse_events(&bytes);
        assert_eq!(
            events[0].kind,
            EventKind::Abs {
                code: 0x35,
                value: 1024
            }
        );
    }

    #[test]
    fn parses_syn_report() {
        let bytes = event_bytes(EV_SYN, SYN_REPORT, 0, 0, 0);
        let events = parse_events(&bytes);
        assert_eq!(events[0].kind, EventKind::Syn);
    }

    #[test]
    fn unknown_type_falls_back() {
        let bytes = event_bytes(0x1f, 7, -1, 0, 0);
        let events = parse_events(&bytes);
        assert_eq!(
            events[0].kind,
            EventKind::Other {
                kind: 0x1f,
                code: 7,
                value: -1
            }
        );
    }

    #[test]
    fn ignores_partial_tail() {
        let mut bytes = event_bytes(EV_KEY, 30, 1, 0, 0);
        bytes.push(0xff);
        let events = parse_events(&bytes);
        assert_eq!(events.len(), 1);
    }
}
