use crate::at::{AtChannel, AtResponse, AtStatus};
use std::io;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PinState {
    #[default]
    Unknown,
    Ready,
    SimPin,
    SimPuk,
}

impl PinState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "READY",
            Self::SimPin => "SIM PIN",
            Self::SimPuk => "SIM PUK",
            Self::Unknown => "UNKNOWN",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value.trim() {
            "READY" => Self::Ready,
            "SIM PIN" => Self::SimPin,
            "SIM PUK" => Self::SimPuk,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Registration {
    #[default]
    Unknown,
    NotRegistered,
    Searching,
    RegisteredHome,
    RegisteredRoaming,
}

impl Registration {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RegisteredHome => "home",
            Self::RegisteredRoaming => "roaming",
            Self::Searching => "searching",
            Self::NotRegistered => "none",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(value: &str) -> Self {
        let stat = value.trim().split(',').next_back().unwrap_or("");
        match stat {
            "0" => Self::NotRegistered,
            "1" => Self::RegisteredHome,
            "2" => Self::Searching,
            "5" => Self::RegisteredRoaming,
            _ => Self::Unknown,
        }
    }
}

pub fn parse_rssi(value: &str) -> Option<u8> {
    let rssi = value.trim().split(',').next()?.trim();
    if rssi == "99" {
        return None;
    }
    rssi.parse().ok()
}

pub fn rssi_dbm(rssi: u8) -> i32 {
    -113 + 2 * i32::from(rssi)
}

#[derive(Debug, Clone, Default)]
pub struct ModemState {
    pub device: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub pin: PinState,
    pub rssi: Option<u8>,
    pub registration: Registration,
}

pub fn open_first(devices: &[String]) -> io::Result<(AtChannel, String)> {
    let mut last = io::Error::new(io::ErrorKind::NotFound, "no modem device in list");
    for path in devices {
        match AtChannel::open(path) {
            Ok(channel) => return Ok((channel, path.clone())),
            Err(e) => last = e,
        }
    }
    Err(last)
}

fn command(channel: &mut AtChannel, name: &str, timeout_ms: u64) -> io::Result<AtResponse> {
    let response = channel.send_command(name, timeout_ms)?;
    if response.status == AtStatus::Ok {
        Ok(response)
    } else {
        Err(io::Error::other(format!(
            "modem rejected {name}: {response:?}"
        )))
    }
}

pub fn probe<'a>(
    channel: &mut AtChannel,
    state: &'a mut ModemState,
    timeout_ms: u64,
) -> io::Result<&'a ModemState> {
    command(channel, "ATE0", timeout_ms)?;
    let manufacturer = command(channel, "AT+CGMI", timeout_ms)?
        .plain()
        .ok_or_else(|| io::Error::other("manufacturer missing"))?;
    let model = command(channel, "AT+CGMM", timeout_ms)?
        .plain()
        .ok_or_else(|| io::Error::other("model missing"))?;
    let pin_raw = command(channel, "AT+CPIN?", timeout_ms)?
        .field("+CPIN:")
        .ok_or_else(|| io::Error::other("pin state missing"))?;
    let csq_raw = command(channel, "AT+CSQ", timeout_ms)?
        .field("+CSQ:")
        .ok_or_else(|| io::Error::other("signal missing"))?;
    let creg_raw = command(channel, "AT+CREG?", timeout_ms)?
        .field("+CREG:")
        .ok_or_else(|| io::Error::other("registration missing"))?;
    let rssi = parse_rssi(&csq_raw).ok_or_else(|| io::Error::other("signal unknown"))?;
    state.manufacturer = Some(manufacturer);
    state.model = Some(model);
    state.pin = PinState::parse(&pin_raw);
    state.rssi = Some(rssi);
    state.registration = Registration::parse(&creg_raw);
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::at::write_all;

    #[test]
    fn parses_pin_states() {
        assert_eq!(PinState::parse("READY"), PinState::Ready);
        assert_eq!(PinState::parse("SIM PIN"), PinState::SimPin);
        assert_eq!(PinState::parse("SIM PUK"), PinState::SimPuk);
        assert_eq!(PinState::parse("what"), PinState::Unknown);
    }

    #[test]
    fn pin_state_strings() {
        assert_eq!(PinState::Ready.as_str(), "READY");
        assert_eq!(PinState::SimPuk.as_str(), "SIM PUK");
        assert_eq!(PinState::Unknown.as_str(), "UNKNOWN");
    }

    #[test]
    fn parses_registration_states() {
        assert_eq!(Registration::parse("0,1"), Registration::RegisteredHome);
        assert_eq!(Registration::parse("0,5"), Registration::RegisteredRoaming);
        assert_eq!(Registration::parse("0,0"), Registration::NotRegistered);
        assert_eq!(Registration::parse("0,2"), Registration::Searching);
        assert_eq!(Registration::parse("0,4"), Registration::Unknown);
        assert_eq!(Registration::parse(""), Registration::Unknown);
    }

    #[test]
    fn parses_signal_levels() {
        assert_eq!(parse_rssi("21,99"), Some(21));
        assert_eq!(parse_rssi("10,99"), Some(10));
        assert_eq!(parse_rssi("99,99"), None);
        assert_eq!(parse_rssi("x,99"), None);
        assert_eq!(parse_rssi(""), None);
    }

    #[test]
    fn converts_rssi_to_dbm() {
        assert_eq!(rssi_dbm(21), -71);
        assert_eq!(rssi_dbm(0), -113);
        assert_eq!(rssi_dbm(31), -51);
    }

    #[test]
    fn modem_defaults() {
        let state = ModemState::default();
        assert_eq!(state.device, None);
        assert_eq!(state.pin, PinState::Unknown);
        assert_eq!(state.registration, Registration::Unknown);
    }

    #[test]
    fn open_first_reports_missing_devices() {
        let devices = vec![
            String::from("/nonexistent-a"),
            String::from("/nonexistent-b"),
        ];
        let err = open_first(&devices).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn command_fails_on_timeout() {
        let mut fds = [0 as libc::c_int; 2];
        assert_eq!(
            unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) },
            0
        );
        let mut channel = AtChannel::from_fd(fds[0]);
        let err = command(&mut channel, "ATE0", 50).unwrap_err();
        assert!(err.to_string().contains("ATE0"));
    }

    #[test]
    fn full_probe_over_socketpair() {
        let mut fds = [0 as libc::c_int; 2];
        assert_eq!(
            unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) },
            0
        );
        let script = b"\r\nOK\r\n\r\nQuectel\r\n\r\nOK\r\n\r\nEG25-G\r\n\r\nOK\r\n\r\n+CPIN: READY\r\n\r\nOK\r\n\r\n+CSQ: 21,99\r\n\r\nOK\r\n\r\n+CREG: 0,1\r\n\r\nOK\r\n";
        write_all(fds[1], script).unwrap();
        let mut channel = AtChannel::from_fd(fds[0]);
        let mut state = ModemState::default();
        let snapshot = probe(&mut channel, &mut state, 2000).unwrap();
        assert_eq!(snapshot.manufacturer.as_deref(), Some("Quectel"));
        assert_eq!(snapshot.model.as_deref(), Some("EG25-G"));
        assert_eq!(snapshot.pin, PinState::Ready);
        assert_eq!(snapshot.rssi, Some(21));
        assert_eq!(snapshot.registration, Registration::RegisteredHome);
    }

    #[test]
    fn probe_fails_on_modem_error() {
        let mut fds = [0 as libc::c_int; 2];
        assert_eq!(
            unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) },
            0
        );
        write_all(fds[1], b"\r\nERROR\r\n").unwrap();
        let mut channel = AtChannel::from_fd(fds[0]);
        let mut state = ModemState::default();
        let err = probe(&mut channel, &mut state, 2000).unwrap_err();
        assert!(err.to_string().contains("ATE0"));
    }
}
