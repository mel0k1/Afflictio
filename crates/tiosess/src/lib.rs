use lioipc::{Request, Response, SessionInfo};
use serde::Deserialize;
use std::fs::OpenOptions;
use std::io;
use std::os::unix::io::AsRawFd;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

const TTY0: &str = "/dev/tty0";

#[cfg(target_env = "musl")]
const VT_ACTIVATE: libc::c_int = 0x5606 as libc::c_int;
#[cfg(not(target_env = "musl"))]
const VT_ACTIVATE: libc::c_ulong = 0x5606;

#[cfg(target_env = "musl")]
const VT_WAITACTIVE: libc::c_int = 0x5607 as libc::c_int;
#[cfg(not(target_env = "musl"))]
const VT_WAITACTIVE: libc::c_ulong = 0x5607;

#[cfg(target_env = "musl")]
const TIOCSCTTY_REQ: libc::c_int = libc::TIOCSCTTY as libc::c_int;
#[cfg(not(target_env = "musl"))]
const TIOCSCTTY_REQ: libc::c_ulong = libc::TIOCSCTTY as libc::c_ulong;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SessionConfig {
    #[serde(default)]
    pub autologin: Vec<Autologin>,
    #[serde(default)]
    pub socket: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Autologin {
    pub user: String,
    pub command: Vec<String>,
    #[serde(default)]
    pub vt: Option<u32>,
}

impl SessionConfig {
    pub fn parse_config(raw: &str) -> Result<SessionConfig, toml::de::Error> {
        if raw.trim().is_empty() {
            return Ok(SessionConfig::default());
        }
        toml::from_str(raw)
    }
}

pub fn activate_vt(vt: u32) -> io::Result<()> {
    let tty0 = OpenOptions::new().read(true).write(true).open(TTY0)?;
    let rc = unsafe { libc::ioctl(tty0.as_raw_fd(), VT_ACTIVATE, vt as libc::c_int) };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    let rc = unsafe { libc::ioctl(tty0.as_raw_fd(), VT_WAITACTIVE, vt as libc::c_int) };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

struct TrackedSession {
    info: SessionInfo,
    child: Child,
}

pub struct SessionManager {
    config: SessionConfig,
    sessions: Vec<TrackedSession>,
    next_id: u64,
    active_vt: u32,
    keyboard_devices: Arc<AtomicU32>,
}

impl SessionManager {
    pub fn new(config: SessionConfig) -> Self {
        Self {
            config,
            sessions: Vec::new(),
            next_id: 1,
            active_vt: 1,
            keyboard_devices: Arc::new(AtomicU32::new(0)),
        }
    }

    pub fn autostart(&mut self) -> io::Result<()> {
        let entries = self.config.autologin.clone();
        let mut first_error = None;
        for (index, auto) in entries.iter().enumerate() {
            let vt = auto.vt.unwrap_or(index as u32 + 1);
            if let Err(e) = self.start_session(auto.user.clone(), auto.command.clone(), vt) {
                if first_error.is_none() {
                    first_error = Some(e);
                }
            }
        }
        match first_error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    pub fn keyboard_counter(&self) -> Arc<AtomicU32> {
        self.keyboard_devices.clone()
    }

    pub fn keyboard_devices(&self) -> u32 {
        self.keyboard_devices.load(Ordering::Relaxed)
    }

    pub fn active_vt(&self) -> u32 {
        self.active_vt
    }

    fn next_vt(&self) -> u32 {
        self.sessions.iter().map(|s| s.info.vt).max().unwrap_or(1) + 1
    }

    pub fn start_session(&mut self, user: String, command: Vec<String>, vt: u32) -> io::Result<()> {
        if command.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "session command is empty",
            ));
        }
        activate_vt(vt)?;
        let tty = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/dev/tty{vt}"))?;
        let tty_fd = tty.as_raw_fd();
        let mut cmd = Command::new(&command[0]);
        cmd.args(&command[1..])
            .stdin(tty.try_clone()?)
            .stdout(tty.try_clone()?)
            .stderr(tty);
        unsafe {
            cmd.pre_exec(move || {
                if libc::setsid() < 0 {
                    return Err(io::Error::last_os_error());
                }
                if libc::ioctl(tty_fd, TIOCSCTTY_REQ, 0) < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = cmd.spawn()?;
        let id = self.next_id;
        self.next_id += 1;
        self.sessions.push(TrackedSession {
            info: SessionInfo {
                id,
                user,
                command: command.join(" "),
                vt,
                state: String::from("running"),
            },
            child,
        });
        self.active_vt = vt;
        Ok(())
    }

    pub fn refresh(&mut self) {
        for session in &mut self.sessions {
            if session.info.state == "running" {
                if let Ok(Some(_)) = session.child.try_wait() {
                    session.info.state = String::from("exited");
                }
            }
        }
    }

    pub fn status(&self) -> Vec<SessionInfo> {
        self.sessions.iter().map(|s| s.info.clone()).collect()
    }

    pub fn switch_vt(&mut self, vt: u32) -> io::Result<()> {
        if vt == 0 || vt > 12 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "vt must be between 1 and 12",
            ));
        }
        if vt == self.active_vt {
            return Ok(());
        }
        activate_vt(vt)?;
        self.active_vt = vt;
        Ok(())
    }
}

pub fn dispatch(manager: &mut SessionManager, req: Request) -> Response {
    match req {
        Request::Status => {
            manager.refresh();
            Response::Status {
                sessions: manager.status(),
                active_vt: manager.active_vt(),
                keyboard_devices: manager.keyboard_devices(),
            }
        }
        Request::Start { user, command } => {
            let vt = manager.next_vt();
            match manager.start_session(user, command, vt) {
                Ok(()) => Response::Ok,
                Err(e) => Response::Error {
                    message: e.to_string(),
                },
            }
        }
        Request::Switch { vt } => match manager.switch_vt(vt) {
            Ok(()) => Response::Ok,
            Err(e) => Response::Error {
                message: e.to_string(),
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_config_gets_defaults() {
        let config = SessionConfig::parse_config("").unwrap();
        assert!(config.autologin.is_empty());
        assert!(config.socket.is_none());
    }

    #[test]
    fn parses_autologin() {
        let raw = r#"
[[autologin]]
user = "mel0k1"
command = ["/usr/bin/afflictio-shell"]
"#;
        let config = SessionConfig::parse_config(raw).unwrap();
        assert_eq!(config.autologin.len(), 1);
        let auto = &config.autologin[0];
        assert_eq!(auto.user, "mel0k1");
        assert_eq!(auto.command, vec!["/usr/bin/afflictio-shell"]);
        assert_eq!(auto.vt, None);
    }

    #[test]
    fn parses_multiple_autologin_entries() {
        let raw = r#"
[[autologin]]
user = "mel0k1"
command = ["/bin/sh"]

[[autologin]]
user = "mel0k1"
command = ["/bin/sh", "-c", "echo vt2"]
vt = 3
"#;
        let config = SessionConfig::parse_config(raw).unwrap();
        assert_eq!(config.autologin.len(), 2);
        assert_eq!(config.autologin[0].vt, None);
        assert_eq!(config.autologin[1].vt, Some(3));
    }

    #[test]
    fn rejects_single_table_autologin() {
        let raw = r#"
[autologin]
user = "mel0k1"
command = ["/bin/sh"]
"#;
        assert!(SessionConfig::parse_config(raw).is_err());
    }

    #[test]
    fn rejects_empty_command_without_spawning() {
        let mut manager = SessionManager::new(SessionConfig::default());
        let resp = dispatch(
            &mut manager,
            Request::Start {
                user: String::from("root"),
                command: Vec::new(),
            },
        );
        assert!(matches!(resp, Response::Error { .. }));
        assert!(manager.status().is_empty());
    }

    #[test]
    fn status_reports_defaults() {
        let mut manager = SessionManager::new(SessionConfig::default());
        let resp = dispatch(&mut manager, Request::Status);
        match resp {
            Response::Status {
                sessions,
                active_vt,
                keyboard_devices,
            } => {
                assert!(sessions.is_empty());
                assert_eq!(active_vt, 1);
                assert_eq!(keyboard_devices, 0);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn switch_to_active_vt_is_noop() {
        let mut manager = SessionManager::new(SessionConfig::default());
        let resp = dispatch(&mut manager, Request::Switch { vt: 1 });
        assert!(matches!(resp, Response::Ok));
    }

    #[test]
    fn switch_validates_range() {
        let mut manager = SessionManager::new(SessionConfig::default());
        for vt in [0u32, 13, 100] {
            let resp = dispatch(&mut manager, Request::Switch { vt });
            assert!(matches!(resp, Response::Error { .. }));
        }
    }
}
