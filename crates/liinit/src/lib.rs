use serde::Deserialize;
use signal_hook::consts::{SIGCHLD, SIGINT, SIGTERM};
use signal_hook::iterator::Signals;
use std::ffi::CString;
use std::fs;
use std::io;
use std::process::{Child, Command};

const DEFAULT_CONFIG: &str = "/etc/afflictio/init.toml";
const CMDLINE_PARAM: &str = "afflictio.init=";

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceSpec {
    pub name: String,
    pub argv: Vec<String>,
    #[serde(default)]
    pub respawn: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MountSpec {
    pub source: String,
    pub target: String,
    pub fstype: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InitConfig {
    #[serde(default)]
    pub mounts: Vec<MountSpec>,
    #[serde(default)]
    pub services: Vec<ServiceSpec>,
}

impl InitConfig {
    pub fn parse(raw: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(raw)
    }
}

pub fn init_param(cmdline: &str) -> Option<String> {
    cmdline
        .split_whitespace()
        .find_map(|p| p.strip_prefix(CMDLINE_PARAM).map(|v| v.to_string()))
}

pub struct Supervisor {
    services: Vec<ServiceSpec>,
    children: Vec<(ServiceSpec, Child)>,
}

impl Supervisor {
    pub fn new(config: InitConfig) -> Self {
        Self {
            services: config.services,
            children: Vec::new(),
        }
    }

    pub fn start(&mut self) -> io::Result<()> {
        for spec in self.services.clone() {
            spawn_spec(&mut self.children, spec)?;
        }
        Ok(())
    }

    pub fn running(&self) -> usize {
        self.children.len()
    }

    pub fn reap(&mut self) {
        loop {
            let mut status = 0;
            let pid = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) };
            if pid <= 0 {
                break;
            }
            let pos = match self.children.iter().position(|(_, c)| c.id() as i32 == pid) {
                Some(pos) => pos,
                None => continue,
            };
            let (spec, _) = self.children.swap_remove(pos);
            if spec.respawn {
                let children = &mut self.children;
                let _ = spawn_spec(children, spec);
            }
        }
    }

    pub fn shutdown(&mut self) {
        for (_, child) in self.children.iter_mut() {
            let _ = child.kill();
        }
        for (_, child) in self.children.iter_mut() {
            let _ = child.wait();
        }
        self.children.clear();
    }
}

fn spawn_spec(children: &mut Vec<(ServiceSpec, Child)>, spec: ServiceSpec) -> io::Result<()> {
    let Some(program) = spec.argv.first() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("service {} has an empty argv", spec.name),
        ));
    };
    let mut cmd = Command::new(program);
    cmd.args(&spec.argv[1..]);
    let child = cmd.spawn()?;
    children.push((spec, child));
    Ok(())
}

fn mount_one(src: &str, target: &str, fstype: &str) -> io::Result<()> {
    let src = CString::new(src).expect("no interior nul");
    let target = CString::new(target).expect("no interior nul");
    let fstype = CString::new(fstype).expect("no interior nul");
    let rc = unsafe {
        libc::mount(
            src.as_ptr(),
            target.as_ptr(),
            fstype.as_ptr(),
            0,
            std::ptr::null(),
        )
    };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn bootstrap_mounts() {
    let mounts = [
        ("proc", "/proc", "proc"),
        ("sysfs", "/sys", "sysfs"),
        ("devtmpfs", "/dev", "devtmpfs"),
        ("tmpfs", "/run", "tmpfs"),
    ];
    for (src, target, fstype) in mounts {
        let target_dir_exists = fs::metadata(target).is_ok();
        if target_dir_exists {
            let _ = mount_one(src, target, fstype);
        }
    }
}

pub fn apply_mounts(mounts: &[MountSpec]) {
    for spec in mounts {
        if let Err(e) = fs::create_dir_all(&spec.target) {
            eprintln!("liinit: mkdir {}: {e}", spec.target);
            continue;
        }
        match mount_one(&spec.source, &spec.target, &spec.fstype) {
            Ok(()) => eprintln!("liinit: mounted {} on {}", spec.fstype, spec.target),
            Err(e) => eprintln!(
                "liinit: mount {} on {} skipped: {e}",
                spec.fstype, spec.target
            ),
        }
    }
}

pub fn run(config_path: &str) -> io::Result<()> {
    if unsafe { libc::getpid() } == 1 {
        bootstrap_mounts();
    }
    let raw = fs::read_to_string(config_path).unwrap_or_default();
    let config = if raw.trim().is_empty() {
        InitConfig {
            mounts: Vec::new(),
            services: Vec::new(),
        }
    } else {
        InitConfig::parse(&raw)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?
    };
    apply_mounts(&config.mounts);
    let mut supervisor = Supervisor::new(config);
    supervisor.start()?;
    let mut signals = Signals::new([SIGCHLD, SIGTERM, SIGINT]).map_err(io::Error::other)?;
    for signal in signals.forever() {
        match signal {
            SIGCHLD => supervisor.reap(),
            SIGTERM | SIGINT => {
                supervisor.shutdown();
                if unsafe { libc::getpid() } == 1 {
                    unsafe { libc::reboot(libc::LINUX_REBOOT_CMD_POWER_OFF) };
                }
                return Ok(());
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn default_config_path() -> String {
    let cmdline = fs::read_to_string("/proc/cmdline").unwrap_or_default();
    init_param(&cmdline).unwrap_or_else(|| DEFAULT_CONFIG.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_config() {
        let raw = r#"
[[mounts]]
source = "binder"
target = "/dev/binderfs"
fstype = "binder"

[[services]]
name = "sessiond"
argv = ["/usr/bin/tiosessd"]
respawn = true

[[services]]
name = "oneshot"
argv = ["/bin/true"]
"#;
        let config = InitConfig::parse(raw).unwrap();
        assert_eq!(config.services.len(), 2);
        assert_eq!(config.mounts.len(), 1);
        assert!(config.services[0].respawn);
        assert!(!config.services[1].respawn);
        assert_eq!(config.services[0].argv[0], "/usr/bin/tiosessd");
        assert_eq!(config.mounts[0].fstype, "binder");
        assert_eq!(config.mounts[0].target, "/dev/binderfs");
    }

    #[test]
    fn empty_config_is_valid() {
        let config = InitConfig::parse("").unwrap();
        assert!(config.services.is_empty());
        assert!(config.mounts.is_empty());
    }

    #[test]
    fn extracts_cmdline_param() {
        let cmdline = "quiet console=ttyS0 afflictio.init=/data/init.toml";
        assert_eq!(init_param(cmdline).as_deref(), Some("/data/init.toml"));
    }

    #[test]
    fn missing_param_is_none() {
        assert_eq!(init_param("quiet splash"), None);
    }

    #[test]
    fn starts_and_counts_children() {
        let config = InitConfig {
            mounts: Vec::new(),
            services: vec![ServiceSpec {
                name: String::from("sleeper"),
                argv: vec![String::from("sleep"), String::from("30")],
                respawn: false,
            }],
        };
        let mut supervisor = Supervisor::new(config);
        supervisor.start().unwrap();
        assert_eq!(supervisor.running(), 1);
        supervisor.shutdown();
        assert_eq!(supervisor.running(), 0);
    }

    #[test]
    fn rejects_empty_argv() {
        let config = InitConfig {
            mounts: Vec::new(),
            services: vec![ServiceSpec {
                name: String::from("broken"),
                argv: Vec::new(),
                respawn: false,
            }],
        };
        let mut supervisor = Supervisor::new(config);
        assert!(supervisor.start().is_err());
    }
}
