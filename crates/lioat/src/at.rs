use std::io;
use std::os::unix::io::RawFd;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtStatus {
    Ok,
    Error,
    Timeout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtResponse {
    pub lines: Vec<String>,
    pub status: AtStatus,
}

impl AtResponse {
    pub fn field(&self, prefix: &str) -> Option<String> {
        self.lines.iter().find_map(|line| {
            line.strip_prefix(prefix)
                .map(|rest| rest.trim().to_string())
        })
    }

    pub fn plain(&self) -> Option<String> {
        self.lines
            .iter()
            .find(|line| !line.starts_with('+'))
            .cloned()
    }
}

pub fn normalize(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_string()
}

pub fn is_final(line: &str) -> Option<AtStatus> {
    if line == "OK" {
        return Some(AtStatus::Ok);
    }
    if line == "ERROR" || line.starts_with("+CME ERROR") || line.starts_with("+CMS ERROR") {
        return Some(AtStatus::Error);
    }
    None
}

pub fn strip_echo(command: &str, lines: &[String]) -> Vec<String> {
    let mut kept = Vec::with_capacity(lines.len());
    let mut echo_removed = false;
    for line in lines {
        if !echo_removed && line == command {
            echo_removed = true;
            continue;
        }
        kept.push(line.clone());
    }
    kept
}

pub struct AtChannel {
    fd: RawFd,
    owned: bool,
    pending: Vec<u8>,
}

impl std::fmt::Debug for AtChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AtChannel").field("fd", &self.fd).finish()
    }
}

impl AtChannel {
    pub fn open(path: &str) -> io::Result<Self> {
        let fd = open_serial(path)?;
        match configure(fd) {
            Ok(()) => Ok(Self {
                fd,
                owned: true,
                pending: Vec::new(),
            }),
            Err(e) => {
                unsafe { libc::close(fd) };
                Err(e)
            }
        }
    }

    pub fn from_fd(fd: RawFd) -> Self {
        Self {
            fd,
            owned: false,
            pending: Vec::new(),
        }
    }

    pub fn send_command(&mut self, command: &str, timeout_ms: u64) -> io::Result<AtResponse> {
        let mut frame = Vec::with_capacity(command.len() + 1);
        frame.extend_from_slice(command.as_bytes());
        frame.push(b'\r');
        write_all(self.fd, &frame)?;
        let mut lines = Vec::new();
        let mut status = AtStatus::Timeout;
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        while status == AtStatus::Timeout {
            if let Some(final_status) = self.drain_lines(&mut lines) {
                status = final_status;
                break;
            }
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            let mut fds = [libc::pollfd {
                fd: self.fd,
                events: libc::POLLIN,
                revents: 0,
            }];
            let remain = (deadline - now).as_millis() as i32;
            let rc = unsafe { libc::poll(fds.as_mut_ptr(), 1, remain) };
            if rc < 0 {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e);
            }
            if rc == 0 {
                break;
            }
            let mut chunk = [0u8; 512];
            let n = unsafe { libc::read(self.fd, chunk.as_mut_ptr().cast(), chunk.len()) };
            if n < 0 {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e);
            }
            if n == 0 {
                break;
            }
            self.pending.extend_from_slice(&chunk[..n as usize]);
        }
        Ok(AtResponse {
            lines: strip_echo(command, &lines),
            status,
        })
    }

    fn drain_lines(&mut self, lines: &mut Vec<String>) -> Option<AtStatus> {
        while let Some(pos) = self.pending.iter().position(|byte| *byte == b'\n') {
            let raw: Vec<u8> = self.pending.drain(..=pos).collect();
            let line = normalize(&raw);
            if line.is_empty() {
                continue;
            }
            if let Some(final_status) = is_final(&line) {
                return Some(final_status);
            }
            lines.push(line);
        }
        None
    }
}

impl Drop for AtChannel {
    fn drop(&mut self) {
        if self.owned {
            unsafe { libc::close(self.fd) };
        }
    }
}

pub(crate) fn write_all(fd: RawFd, mut data: &[u8]) -> io::Result<()> {
    while !data.is_empty() {
        let n = unsafe { libc::write(fd, data.as_ptr().cast(), data.len()) };
        if n < 0 {
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e);
        }
        data = &data[n as usize..];
    }
    Ok(())
}

fn open_serial(path: &str) -> io::Result<RawFd> {
    let cpath = std::ffi::CString::new(path)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "device path has interior nul"))?;
    let fd = unsafe {
        libc::open(
            cpath.as_ptr(),
            libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC | libc::O_NONBLOCK,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(fd)
}

fn configure(fd: RawFd) -> io::Result<()> {
    let mut termios: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut termios) } != 0 {
        return Err(io::Error::last_os_error());
    }
    unsafe { libc::cfmakeraw(&mut termios) };
    termios.c_cflag |= libc::CLOCAL | libc::CREAD;
    if unsafe { libc::cfsetispeed(&mut termios, libc::B115200) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::cfsetospeed(&mut termios, libc::B115200) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &termios) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn final_ok() {
        assert_eq!(is_final("OK"), Some(AtStatus::Ok));
    }

    #[test]
    fn final_error_variants() {
        assert_eq!(is_final("ERROR"), Some(AtStatus::Error));
        assert_eq!(is_final("+CME ERROR: 10"), Some(AtStatus::Error));
        assert_eq!(is_final("+CMS ERROR: 300"), Some(AtStatus::Error));
        assert_eq!(is_final("+CPIN: READY"), None);
    }

    #[test]
    fn normalizes_lines() {
        assert_eq!(normalize(b"\r\nQuectel\r\n"), "Quectel");
        assert_eq!(normalize(b"\r\n\r\n"), "");
        assert_eq!(normalize(b"garbage \r\n"), "garbage");
        assert_eq!(normalize(&[0xff]), "\u{fffd}");
    }

    #[test]
    fn strips_echo_only_first_match() {
        let lines = vec![
            String::from("AT+CGMM"),
            String::from("EG25-G"),
            String::from("EG25-G"),
        ];
        assert_eq!(strip_echo("AT+CGMM", &lines), ["EG25-G", "EG25-G"]);
    }

    #[test]
    fn keeps_urc_without_echo() {
        let lines = vec![String::from("+CSQ: 21,99"), String::from("OK")];
        assert_eq!(strip_echo("AT+CSQ", &lines), ["+CSQ: 21,99", "OK"]);
    }

    #[test]
    fn extracts_field_and_plain_lines() {
        let response = AtResponse {
            lines: vec![String::from("+CPIN: READY"), String::from("Quectel")],
            status: AtStatus::Ok,
        };
        assert_eq!(response.field("+CPIN:").as_deref(), Some("READY"));
        assert_eq!(response.plain().as_deref(), Some("Quectel"));
    }

    #[test]
    fn reads_response_over_socketpair() {
        let mut fds = [0 as libc::c_int; 2];
        assert_eq!(
            unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) },
            0
        );
        let mut channel = AtChannel::from_fd(fds[0]);
        let peer = fds[1];
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            write_all(peer, b"\r\nOK\r\n").unwrap();
        });
        let response = channel.send_command("AT", 2000).unwrap();
        assert_eq!(response.status, AtStatus::Ok);
        assert!(response.lines.is_empty());
    }

    #[test]
    fn collects_urc_lines_over_socketpair() {
        let mut fds = [0 as libc::c_int; 2];
        assert_eq!(
            unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) },
            0
        );
        let mut channel = AtChannel::from_fd(fds[0]);
        let peer = fds[1];
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            write_all(peer, b"\r\n+CSQ: 21,99\r\n\r\nOK\r\n").unwrap();
        });
        let response = channel.send_command("AT+CSQ", 2000).unwrap();
        assert_eq!(response.status, AtStatus::Ok);
        assert_eq!(response.lines, ["+CSQ: 21,99"]);
    }

    #[test]
    fn echo_removed_over_socketpair() {
        let mut fds = [0 as libc::c_int; 2];
        assert_eq!(
            unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) },
            0
        );
        let mut channel = AtChannel::from_fd(fds[0]);
        let peer = fds[1];
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            write_all(peer, b"AT+CGMM\r\nEG25-G\r\n\r\nOK\r\n").unwrap();
        });
        let response = channel.send_command("AT+CGMM", 2000).unwrap();
        assert_eq!(response.status, AtStatus::Ok);
        assert_eq!(response.lines, ["EG25-G"]);
    }

    #[test]
    fn times_out_without_reply() {
        let mut fds = [0 as libc::c_int; 2];
        assert_eq!(
            unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) },
            0
        );
        let mut channel = AtChannel::from_fd(fds[0]);
        let response = channel.send_command("AT", 50).unwrap();
        assert_eq!(response.status, AtStatus::Timeout);
        assert!(response.lines.is_empty());
    }
}
