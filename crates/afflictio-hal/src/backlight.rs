use std::fs;
use std::io;

const BACKLIGHT_BASE: &str = "/sys/class/backlight";

#[derive(Debug, Clone)]
pub struct Backlight {
    pub name: String,
    base: String,
}

impl Backlight {
    pub fn list() -> io::Result<Vec<Backlight>> {
        let mut out = Vec::new();
        for entry in fs::read_dir(BACKLIGHT_BASE)? {
            let entry = entry?;
            out.push(Backlight {
                name: entry.file_name().to_string_lossy().into_owned(),
                base: entry.path().to_string_lossy().into_owned(),
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    pub fn brightness(&self) -> io::Result<u32> {
        parse_u32(&fs::read_to_string(self.path("brightness"))?)
    }

    pub fn max_brightness(&self) -> io::Result<u32> {
        parse_u32(&fs::read_to_string(self.path("max_brightness"))?)
    }

    pub fn set_brightness(&self, value: u32) -> io::Result<()> {
        let max = self.max_brightness().unwrap_or(value);
        let value = value.min(max);
        fs::write(self.path("brightness"), format!("{value}\n"))
    }

    fn path(&self, file: &str) -> String {
        format!("{}/{}", self.base, file)
    }
}

pub fn parse_u32(raw: &str) -> io::Result<u32> {
    raw.trim()
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "not an unsigned number"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_numbers() {
        assert_eq!(parse_u32("1024\n").unwrap(), 1024);
        assert_eq!(parse_u32(" 0 ").unwrap(), 0);
        assert!(parse_u32("-1").is_err());
        assert!(parse_u32("abc").is_err());
    }
}
