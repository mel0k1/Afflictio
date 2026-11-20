use serde::Deserialize;

pub const DEFAULT_CONFIG: &str = "/etc/afflictio/slots.toml";
pub const DEFAULT_BOOT_MOUNT: &str = "/boot";
pub const DEFAULT_BOOT_CONF: &str = "/boot/extlinux/extlinux.conf";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SlotSpec {
    pub name: String,
    pub partition: u8,
    pub uuid: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SlotsConfig {
    pub disk: String,
    pub boot_partition: u8,
    #[serde(default)]
    pub boot_mount: Option<String>,
    #[serde(default)]
    pub boot_conf: Option<String>,
    #[serde(default)]
    pub boot_ok: Option<String>,
    #[serde(default)]
    pub slots: Vec<SlotSpec>,
}

impl SlotsConfig {
    pub fn parse(raw: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(raw)
    }

    pub fn partition_path(&self, index: u8) -> String {
        let infix = if self.disk.chars().last().is_some_and(|c| c.is_ascii_digit()) {
            "p"
        } else {
            ""
        };
        format!("{}{}{}", self.disk, infix, index)
    }

    pub fn boot_device(&self) -> String {
        self.partition_path(self.boot_partition)
    }

    pub fn boot_mount(&self) -> String {
        self.boot_mount
            .clone()
            .unwrap_or_else(|| DEFAULT_BOOT_MOUNT.to_string())
    }

    pub fn boot_conf(&self) -> String {
        self.boot_conf
            .clone()
            .unwrap_or_else(|| DEFAULT_BOOT_CONF.to_string())
    }

    pub fn boot_ok_path(&self) -> String {
        self.boot_ok
            .clone()
            .unwrap_or_else(|| super::ops::DEFAULT_BOOT_OK.to_string())
    }

    pub fn by_name(&self, name: &str) -> Option<&SlotSpec> {
        self.slots.iter().find(|slot| slot.name == name)
    }

    pub fn by_uuid(&self, uuid: &str) -> Option<&SlotSpec> {
        self.slots.iter().find(|slot| slot.uuid == uuid)
    }

    pub fn other(&self, name: &str) -> Option<&SlotSpec> {
        if !self.slots.iter().any(|slot| slot.name == name) {
            return None;
        }
        self.slots.iter().find(|slot| slot.name != name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAW: &str = r#"
disk = "/dev/vda"
boot_partition = 1

[[slots]]
name = "a"
partition = 2
uuid = "11111111-1111-1111-1111-111111111111"

[[slots]]
name = "b"
partition = 3
uuid = "22222222-2222-2222-2222-222222222222"
"#;

    #[test]
    fn parses_config() {
        let config = SlotsConfig::parse(RAW).unwrap();
        assert_eq!(config.disk, "/dev/vda");
        assert_eq!(config.boot_partition, 1);
        assert_eq!(config.slots.len(), 2);
        assert_eq!(config.slots[0].name, "a");
        assert_eq!(config.slots[0].partition, 2);
        assert_eq!(config.slots[1].name, "b");
        assert_eq!(config.boot_device(), "/dev/vda1");
        assert_eq!(config.boot_mount(), "/boot");
        assert_eq!(config.boot_conf(), "/boot/extlinux/extlinux.conf");
        assert_eq!(config.boot_ok_path(), "/var/lib/afflictio/boot-ok");
    }

    #[test]
    fn finds_by_name_and_uuid() {
        let config = SlotsConfig::parse(RAW).unwrap();
        assert!(config.by_name("a").is_some());
        assert!(config.by_name("z").is_none());
        assert_eq!(
            config
                .by_uuid("22222222-2222-2222-2222-222222222222")
                .unwrap()
                .name,
            "b"
        );
    }

    #[test]
    fn finds_the_other_slot() {
        let config = SlotsConfig::parse(RAW).unwrap();
        assert_eq!(config.other("a").unwrap().name, "b");
        assert_eq!(config.other("b").unwrap().name, "a");
        assert!(config.other("z").is_none());
    }

    #[test]
    fn partition_paths_follow_kernel_naming() {
        let mut config = SlotsConfig::parse(RAW).unwrap();
        assert_eq!(config.partition_path(3), "/dev/vda3");
        config.disk = String::from("/dev/mmcblk0");
        assert_eq!(config.partition_path(1), "/dev/mmcblk0p1");
        assert_eq!(config.partition_path(12), "/dev/mmcblk0p12");
    }

    #[test]
    fn invalid_toml_fails() {
        assert!(SlotsConfig::parse("disk = [").is_err());
    }

    #[test]
    fn empty_slots_allowed() {
        let config = SlotsConfig::parse("disk = \"/dev/vda\"\nboot_partition = 1").unwrap();
        assert!(config.slots.is_empty());
        assert!(config.other("a").is_none());
        assert_eq!(config.boot_device(), "/dev/vda1");
    }
}
