use lioipc as ipc;
use lioipc::{Request, Response};
use std::fmt::Write as _;
use tiohal as hal;

pub enum Command {
    Help,
    Status,
    Devices,
    Poweroff,
    Reboot,
    SwitchVt { vt: u32 },
    SessionStart { user: String, command: Vec<String> },
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    let Some(sub) = args.first() else {
        return Ok(Command::Help);
    };
    match sub.as_str() {
        "help" | "--help" | "-h" => Ok(Command::Help),
        "status" => Ok(Command::Status),
        "devices" => Ok(Command::Devices),
        "poweroff" => Ok(Command::Poweroff),
        "reboot" => Ok(Command::Reboot),
        "switch" => match args.get(1).and_then(|v| v.parse::<u32>().ok()) {
            Some(vt) => Ok(Command::SwitchVt { vt }),
            None => Err(usage("switch")),
        },
        "session" => {
            if args.len() < 3 {
                return Err(usage("session"));
            }
            Ok(Command::SessionStart {
                user: args[1].clone(),
                command: args[2..].to_vec(),
            })
        }
        other => Err(format!("unknown command: {other}\n{}", usage("help"))),
    }
}

fn usage(sub: &str) -> String {
    match sub {
        "switch" => String::from("usage: tioctl switch <vt>"),
        "session" => String::from("usage: tioctl session <user> <command> [args...]"),
        _ => String::from(
            "usage: tioctl <command>\n\ncommands:\n  status\n  devices\n  session <user> <command> [args...]\n  switch <vt>\n  poweroff\n  reboot\n  help",
        ),
    }
}

pub fn run(command: Command) -> Result<(), String> {
    match command {
        Command::Help => {
            println!("{}", usage("help"));
            Ok(())
        }
        Command::Status => status(),
        Command::Devices => devices(),
        Command::Poweroff => power(libc::LINUX_REBOOT_CMD_POWER_OFF),
        Command::Reboot => power(libc::LINUX_REBOOT_CMD_RESTART),
        Command::SwitchVt { vt } => send(&Request::Switch { vt }).map(print_response),
        Command::SessionStart { user, command } => {
            send(&Request::Start { user, command }).map(print_response)
        }
    }
}

fn status() -> Result<(), String> {
    match ipc::request(&Request::Status) {
        Ok(resp) => {
            print_response(resp);
            Ok(())
        }
        Err(_) => {
            print_local_status();
            Ok(())
        }
    }
}

fn print_local_status() {
    println!("sessiond: offline");
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
    println!("kernel: {}", release.trim());
    let uptime = std::fs::read_to_string("/proc/uptime").unwrap_or_default();
    if let Some(seconds) = uptime.split_whitespace().next() {
        println!("uptime: {} s", seconds.split('.').next().unwrap_or(seconds));
    }
}

fn devices() -> Result<(), String> {
    let paths = hal::input::device_paths().map_err(|e| e.to_string())?;
    for path in paths {
        match hal::input::InputDevice::open(&path) {
            Ok(dev) => println!("input {} {}", dev.path(), dev.name()),
            Err(_) => println!("input {path} (access denied)"),
        }
    }
    for battery in hal::power::batteries().map_err(|e| e.to_string())? {
        println!(
            "battery {} {}% {}",
            battery.name, battery.capacity, battery.status
        );
    }
    for backlight in hal::backlight::Backlight::list().map_err(|e| e.to_string())? {
        let current = backlight
            .brightness()
            .map(|v| v.to_string())
            .unwrap_or_else(|_| String::from("?"));
        let max = backlight
            .max_brightness()
            .map(|v| v.to_string())
            .unwrap_or_else(|_| String::from("?"));
        println!("backlight {} {} / {}", backlight.name, current, max);
    }
    Ok(())
}

fn power(cmd: libc::c_int) -> Result<(), String> {
    let rc = unsafe { libc::reboot(cmd) };
    if rc < 0 {
        return Err(format!(
            "reboot(2) failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

fn send(req: &Request) -> Result<Response, String> {
    ipc::request(req).map_err(|e| format!("sessiond unreachable: {e}"))
}

fn print_response(resp: Response) {
    match serde_json::to_string_pretty(&resp) {
        Ok(text) => println!("{text}"),
        Err(e) => println!("response: {e}"),
    }
}

pub fn help_text() -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{}", usage("help"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn no_args_shows_help() {
        assert!(matches!(parse(&[]), Ok(Command::Help)));
        assert!(matches!(parse(&args(&["-h"])), Ok(Command::Help)));
    }

    #[test]
    fn simple_commands() {
        assert!(matches!(parse(&args(&["status"])), Ok(Command::Status)));
        assert!(matches!(parse(&args(&["devices"])), Ok(Command::Devices)));
        assert!(matches!(parse(&args(&["poweroff"])), Ok(Command::Poweroff)));
        assert!(matches!(parse(&args(&["reboot"])), Ok(Command::Reboot)));
    }

    #[test]
    fn parses_switch_vt() {
        match parse(&args(&["switch", "3"])) {
            Ok(Command::SwitchVt { vt }) => assert_eq!(vt, 3),
            _ => panic!("expected SwitchVt"),
        }
        assert!(parse(&args(&["switch"])).is_err());
        assert!(parse(&args(&["switch", "abc"])).is_err());
    }

    #[test]
    fn parses_session_start() {
        match parse(&args(&["session", "mel0k1", "/bin/sh", "-l"])) {
            Ok(Command::SessionStart { user, command }) => {
                assert_eq!(user, "mel0k1");
                assert_eq!(command, vec!["/bin/sh", "-l"]);
            }
            _ => panic!("expected SessionStart"),
        }
        assert!(parse(&args(&["session", "mel0k1"])).is_err());
    }

    #[test]
    fn unknown_command_errors() {
        assert!(parse(&args(&["fly"])).is_err());
    }

    #[test]
    fn help_text_mentions_commands() {
        let text = help_text();
        assert!(text.contains("status"));
        assert!(text.contains("session"));
    }
}
