use afflictio_ipc::{Request, Response, SessionInfo};
use serde::Deserialize;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::io::AsRawFd;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command};

const TTY0: &str = "/dev/tty0";
const VT_ACTIVATE: libc::c_ulong = 0x5606;
const VT_WAITACTIVE: libc::c_ulong = 0x5607;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SessionConfig {
    #[serde(default)]
    pub autologin: Option<Autologin>,
    #[serde(default)]
    pub socket: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Autologin {
    pub user: String,
    pub command: Vec<String>,
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
}

impl SessionManager {
    pub fn new(config: SessionConfig) -> Self {
        Self {
            config,
            sessions: Vec::new(),
            next_id: 1,
            active_vt: 1,
        }
    }

    pub fn autostart(&mut self) -> io::Result<()> {
        let Some(auto) = self.config.autologin.clone() else {
            return Ok(());
        };
        self.start_session(auto.user, auto.command, 1)
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
        let tty = File::open(format!("/dev/tty{vt}"))?;
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
                if libc::ioctl(tty_fd, libc::TIOCSCTTY as libc::c_ulong, 0) < 0 {
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
        assert!(config.autologin.is_none());
        assert!(config.socket.is_none());
    }

    #[test]
    fn parses_autologin() {
        let raw = r#"
[autologin]
user = "mel0k1"
command = ["/usr/bin/afflictio-shell"]
"#;
        let config = SessionConfig::parse_config(raw).unwrap();
        let auto = config.autologin.expect("autologin present");
        assert_eq!(auto.user, "mel0k1");
        assert_eq!(auto.command, vec!["/usr/bin/afflictio-shell"]);
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
            } => {
                assert!(sessions.is_empty());
                assert_eq!(active_vt, 1);
            }
            _ => panic!("wrong variant"),
        }
    }
}
