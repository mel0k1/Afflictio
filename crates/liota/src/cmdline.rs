pub const ROOT_PREFIX: &str = "root=UUID=";

pub fn active_uuid(cmdline: &str) -> Option<String> {
    cmdline.split_whitespace().find_map(|param| {
        param
            .strip_prefix(ROOT_PREFIX)
            .map(|uuid| uuid.trim().to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_root_uuid() {
        let cmdline = "console=tty1 console=ttyS0,115200 root=UUID=aff1a001-0505-4a0b-95c1-1a2b3c4d5e6f rootwait init=/sbin/liinit";
        assert_eq!(
            active_uuid(cmdline).as_deref(),
            Some("aff1a001-0505-4a0b-95c1-1a2b3c4d5e6f")
        );
    }

    #[test]
    fn missing_root_is_none() {
        assert_eq!(active_uuid("console=tty1 quiet"), None);
        assert_eq!(active_uuid(""), None);
    }

    #[test]
    fn plain_root_path_is_none() {
        assert_eq!(active_uuid("root=/dev/vda2"), None);
        assert_eq!(active_uuid("root=UUID"), None);
    }
}
