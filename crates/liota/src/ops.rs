use crate::cfg::SlotsConfig;
use crate::cmdline;
use crate::extlinux;
use std::ffi::CString;
use std::fs;
use std::io;
use std::path::Path;

pub const DEFAULT_BOOT_OK: &str = "/var/lib/afflictio/boot-ok";
const COPY_CHUNK: usize = 4 * 1024 * 1024;

pub fn boot_ok_path(config: &SlotsConfig) -> String {
    config.boot_ok_path()
}

pub fn status(config: &SlotsConfig, cmdline: &str) -> io::Result<StatusReport> {
    let uuid = cmdline::active_uuid(cmdline).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "cmdline carries no root=UUID=")
    })?;
    let active = config.by_uuid(&uuid).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("active uuid {uuid} is not in the slot list"),
        )
    })?;
    let inactive = config.other(&active.name).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "the slot list needs a second slot")
    })?;
    Ok(StatusReport {
        active: active.name.clone(),
        inactive: inactive.name.clone(),
        boot_ok: Path::new(&boot_ok_path(config)).exists(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StatusReport {
    pub active: String,
    pub inactive: String,
    pub boot_ok: bool,
}

pub fn switch(config: &SlotsConfig, cmdline: &str) -> io::Result<String> {
    let uuid = cmdline::active_uuid(cmdline).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "cmdline carries no root=UUID=")
    })?;
    let active = config.by_uuid(&uuid).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("active uuid {uuid} is not in the slot list"),
        )
    })?;
    let target = config
        .other(&active.name)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no second slot to switch to"))?;
    mount_boot(config)?;
    let result = rewrite_boot_conf(config, &target.name);
    let _ = umount_boot(config);
    result?;
    Ok(target.name.clone())
}

pub fn rewrite_boot_conf(config: &SlotsConfig, target_label: &str) -> io::Result<()> {
    let conf_path = config.boot_conf();
    let conf = fs::read_to_string(&conf_path)?;
    if extlinux::default_label(&conf).is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "extlinux.conf carries no default label",
        ));
    }
    let rewritten = extlinux::set_default(&conf, target_label);
    let tmp = format!("{conf_path}.tmp");
    fs::write(&tmp, rewritten.as_bytes())?;
    fs::rename(&tmp, &conf_path)?;
    Ok(())
}

pub fn install(payload: &str, partition: &str) -> io::Result<u64> {
    let mut source = fs::File::open(payload)?;
    let mut target = fs::OpenOptions::new().write(true).open(partition)?;
    let mut total = 0u64;
    let mut buf = vec![0u8; COPY_CHUNK];
    loop {
        let n = io::Read::read(&mut source, &mut buf)?;
        if n == 0 {
            break;
        }
        io::Write::write_all(&mut target, &buf[..n])?;
        total += n as u64;
    }
    target.sync_all()?;
    Ok(total)
}

fn mount_boot(config: &SlotsConfig) -> io::Result<()> {
    let mount = config.boot_mount();
    fs::create_dir_all(&mount)?;
    let src = CString::new(config.boot_device()).expect("no interior nul");
    let target = CString::new(mount).expect("no interior nul");
    let rc = unsafe {
        libc::mount(
            src.as_ptr(),
            target.as_ptr(),
            c"ext4".as_ptr(),
            0,
            std::ptr::null(),
        )
    };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn umount_boot(config: &SlotsConfig) -> io::Result<()> {
    let target = CString::new(config.boot_mount()).expect("no interior nul");
    let rc = unsafe { libc::umount(target.as_ptr()) };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::SlotSpec;

    fn temp_config(dir: &Path, conf: &str) -> SlotsConfig {
        let conf_path = dir.join("extlinux/extlinux.conf");
        fs::create_dir_all(conf_path.parent().unwrap()).unwrap();
        fs::write(&conf_path, conf).unwrap();
        SlotsConfig {
            disk: String::from("/dev/vda"),
            boot_partition: 1,
            boot_mount: Some(dir.join("boot").to_string_lossy().to_string()),
            boot_conf: Some(conf_path.to_string_lossy().to_string()),
            boot_ok: Some(dir.join("boot-ok").to_string_lossy().to_string()),
            slots: vec![
                SlotSpec {
                    name: String::from("a"),
                    partition: 2,
                    uuid: String::from("1111-aaaa"),
                },
                SlotSpec {
                    name: String::from("b"),
                    partition: 3,
                    uuid: String::from("2222-bbbb"),
                },
            ],
        }
    }

    const CONF: &str = "default a\ntimeout 0\nlabel a\n  append console=ttyS0 root=UUID=1111-aaaa\nlabel b\n  append console=ttyS0 root=UUID=2222-bbbb\n";

    #[test]
    fn status_reports_active_and_boot_ok() {
        let dir = std::env::temp_dir().join(format!("liota-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let config = temp_config(&dir, CONF);
        let report = status(&config, "root=UUID=1111-aaaa quiet").unwrap();
        assert_eq!(report.active, "a");
        assert_eq!(report.inactive, "b");
        assert!(!report.boot_ok);
        fs::write(boot_ok_path(&config), b"1").unwrap();
        let report = status(&config, "root=UUID=1111-aaaa quiet").unwrap();
        assert!(report.boot_ok);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn status_rejects_unknown_uuid() {
        let dir = std::env::temp_dir().join(format!("liota-test2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let config = temp_config(&dir, CONF);
        let err = status(&config, "root=UUID=9999-zzzz").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        let err = status(&config, "quiet").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rewrite_boot_conf_switches_label_atomically() {
        let dir = std::env::temp_dir().join(format!("liota-test3-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let config = temp_config(&dir, CONF);
        rewrite_boot_conf(&config, "b").unwrap();
        let conf = fs::read_to_string(config.boot_conf()).unwrap();
        assert_eq!(extlinux::default_label(&conf), Some("b"));
        assert!(
            extlinux::label_uuid(extlinux::label_append(&conf, "a").unwrap()).unwrap()
                == "1111-aaaa"
        );
        assert!(!dir.join("extlinux/extlinux.conf.tmp").exists());
        rewrite_boot_conf(&config, "a").unwrap();
        let conf = fs::read_to_string(config.boot_conf()).unwrap();
        assert_eq!(extlinux::default_label(&conf), Some("a"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rewrite_rejects_conf_without_default() {
        let dir = std::env::temp_dir().join(format!("liota-test4-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let config = temp_config(&dir, "label a\n  append root=UUID=1111-aaaa\n");
        let err = rewrite_boot_conf(&config, "b").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn install_copies_all_bytes_and_syncs() {
        let dir = std::env::temp_dir().join(format!("liota-test5-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let payload = dir.join("payload.bin");
        fs::write(&payload, vec![7u8; 100_000]).unwrap();
        let partition = dir.join("partition.img");
        fs::write(&partition, vec![0u8; 250_000]).unwrap();
        let written = install(payload.to_str().unwrap(), partition.to_str().unwrap()).unwrap();
        assert_eq!(written, 100_000);
        let data = fs::read(&partition).unwrap();
        assert_eq!(&data[..100_000], &vec![7u8; 100_000][..]);
        assert_eq!(&data[100_000..], &vec![0u8; 150_000][..]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn switch_fails_without_active_slot_match() {
        let dir = std::env::temp_dir().join(format!("liota-test6-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let config = temp_config(&dir, CONF);
        let err = switch(&config, "root=UUID=9999-zzzz").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        let _ = fs::remove_dir_all(&dir);
    }
}
