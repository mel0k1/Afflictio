pub fn default_label(conf: &str) -> Option<&str> {
    conf.lines()
        .find_map(|line| line.trim().strip_prefix("default "))
        .map(|rest| rest.trim())
}

pub fn label_append<'a>(conf: &'a str, label: &str) -> Option<&'a str> {
    let mut matched = false;
    for line in conf.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("label ") && trimmed[6..].trim() == label {
            matched = true;
            continue;
        }
        if matched {
            if trimmed.starts_with("label ") {
                return None;
            }
            if let Some(rest) = trimmed.strip_prefix("append ") {
                return Some(rest.trim());
            }
        }
    }
    None
}

pub fn label_uuid(append: &str) -> Option<&str> {
    append
        .split_whitespace()
        .find_map(|param| param.strip_prefix("root=UUID="))
}

pub fn set_default(conf: &str, label: &str) -> String {
    let mut out = Vec::new();
    for line in conf.lines() {
        if line.trim_start().starts_with("default ") {
            out.push(format!("default {label}"));
        } else {
            out.push(line.to_string());
        }
    }
    let mut text = out.join("\n");
    if !conf.ends_with('\n') {
        return text;
    }
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONF: &str = "default a\ntimeout 0\n\nlabel a\n  menu label Afflictio slot a\n  linux /Image\n  append console=ttyS0,115200 root=UUID=1111-aaaa rootwait init=/sbin/liinit\n\nlabel b\n  menu label Afflictio slot b\n  linux /Image\n  append console=ttyS0,115200 root=UUID=2222-bbbb rootwait init=/sbin/liinit\n";

    #[test]
    fn reads_default_label() {
        assert_eq!(default_label(CONF), Some("a"));
        assert_eq!(default_label("timeout 0\n"), None);
    }

    #[test]
    fn reads_label_append() {
        let append = label_append(CONF, "b").unwrap();
        assert!(append.contains("root=UUID=2222-bbbb"));
        assert!(label_append(CONF, "z").is_none());
    }

    #[test]
    fn append_search_stops_at_next_label() {
        let conf =
            "default a\nlabel a\n  append root=UUID=1111\nlabel b\n  append root=UUID=2222\n";
        assert_eq!(label_uuid(label_append(conf, "a").unwrap()), Some("1111"));
        assert_eq!(label_uuid(label_append(conf, "b").unwrap()), Some("2222"));
    }

    #[test]
    fn extracts_label_uuid() {
        assert_eq!(
            label_uuid("console=ttyS0 root=UUID=2222-bbbb rootwait"),
            Some("2222-bbbb")
        );
        assert_eq!(label_uuid("console=ttyS0"), None);
    }

    #[test]
    fn switches_default_to_b() {
        let rewritten = set_default(CONF, "b");
        assert_eq!(default_label(&rewritten), Some("b"));
        assert!(rewritten.contains("root=UUID=1111-aaaa"));
        assert!(rewritten.contains("root=UUID=2222-bbbb"));
        assert!(rewritten.ends_with('\n'));
    }

    #[test]
    fn switches_default_back_to_a() {
        let rewritten = set_default(CONF, "a");
        assert_eq!(default_label(&rewritten), Some("a"));
        let other = set_default(&rewritten, "b");
        assert_eq!(default_label(&other), Some("b"));
    }

    #[test]
    fn missing_label_line_is_added_nowhere() {
        let conf = "label a\n  append root=UUID=1111\n";
        let rewritten = set_default(conf, "b");
        assert_eq!(default_label(&rewritten), None);
        assert!(rewritten.contains("root=UUID=1111"));
    }
}
