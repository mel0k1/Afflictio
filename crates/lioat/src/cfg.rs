use serde::Deserialize;

pub const DEFAULT_SOCKET: &str = "/run/afflictio/modem.sock";
const DEFAULT_PROBE_INTERVAL: u64 = 30;
const DEFAULT_RESPONSE_TIMEOUT: u64 = 2000;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ModemConfig {
    #[serde(default)]
    pub devices: Vec<String>,
    #[serde(default)]
    pub socket: Option<String>,
    #[serde(default)]
    pub probe_interval: Option<u64>,
    #[serde(default)]
    pub response_timeout: Option<u64>,
}

impl ModemConfig {
    pub fn parse(raw: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(raw)
    }

    pub fn socket(&self) -> String {
        self.socket
            .clone()
            .unwrap_or_else(|| DEFAULT_SOCKET.to_string())
    }

    pub fn probe_interval(&self) -> u64 {
        self.probe_interval.unwrap_or(DEFAULT_PROBE_INTERVAL)
    }

    pub fn response_timeout(&self) -> u64 {
        self.response_timeout.unwrap_or(DEFAULT_RESPONSE_TIMEOUT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_config() {
        let raw = r#"
devices = ["/dev/ttyUSB2", "/dev/ttyAMA1"]
socket = "/run/afflictio/modem.sock"
probe_interval = 15
response_timeout = 3000
"#;
        let config = ModemConfig::parse(raw).unwrap();
        assert_eq!(config.devices, ["/dev/ttyUSB2", "/dev/ttyAMA1"],);
        assert_eq!(config.socket(), "/run/afflictio/modem.sock");
        assert_eq!(config.probe_interval(), 15);
        assert_eq!(config.response_timeout(), 3000);
    }

    #[test]
    fn empty_config_uses_defaults() {
        let config = ModemConfig::parse("").unwrap();
        assert!(config.devices.is_empty());
        assert_eq!(config.socket(), "/run/afflictio/modem.sock");
        assert_eq!(config.probe_interval(), 30);
        assert_eq!(config.response_timeout(), 2000);
    }

    #[test]
    fn devices_keep_order() {
        let config = ModemConfig::parse("devices = [\"b\", \"a\"]").unwrap();
        assert_eq!(config.devices, ["b", "a"]);
    }

    #[test]
    fn invalid_toml_fails() {
        assert!(ModemConfig::parse("devices = [").is_err());
    }

    #[test]
    fn missing_optional_fields_fall_back() {
        let config = ModemConfig::parse("devices = [\"/dev/ttyAMA1\"]").unwrap();
        assert_eq!(config.devices, ["/dev/ttyAMA1"]);
        assert_eq!(config.socket(), "/run/afflictio/modem.sock");
        assert_eq!(config.probe_interval(), 30);
        assert_eq!(config.response_timeout(), 2000);
    }
}
