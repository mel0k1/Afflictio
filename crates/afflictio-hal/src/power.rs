use std::fs;
use std::io;

#[derive(Debug, Clone)]
pub struct Battery {
    pub name: String,
    pub capacity: u8,
    pub status: String,
}

pub fn batteries() -> io::Result<Vec<Battery>> {
    let mut out = Vec::new();
    for entry in fs::read_dir("/sys/class/power_supply")? {
        let entry = entry?;
        let path = entry.path();
        let kind = fs::read_to_string(path.join("type")).unwrap_or_default();
        if !kind.trim().eq_ignore_ascii_case("battery") {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let capacity = fs::read_to_string(path.join("capacity"))
            .ok()
            .and_then(|raw| parse_capacity(&raw).ok())
            .unwrap_or(0);
        let status =
            fs::read_to_string(path.join("status")).unwrap_or_else(|_| String::from("unknown"));
        out.push(Battery {
            name,
            capacity,
            status: status.trim().to_string(),
        });
    }
    Ok(out)
}

pub fn parse_capacity(raw: &str) -> Result<u8, std::num::ParseIntError> {
    raw.trim().parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_capacity() {
        assert_eq!(parse_capacity("87\n").unwrap(), 87);
        assert_eq!(parse_capacity("  0  ").unwrap(), 0);
    }

    #[test]
    fn rejects_bad_capacity() {
        assert!(parse_capacity("full").is_err());
        assert!(parse_capacity("").is_err());
    }
}
