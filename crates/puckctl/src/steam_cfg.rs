use std::fs;
use std::path::Path;

use crate::log::logln;
use crate::paths::{self, cfgbak_dir};

const EMPTY_769: &str = "\t\"769\"\n\t{\n\t\t\"official\"\t\t\"empty.vdf\"\n\t}\n";
const DESKTOP_769: &str = "\t\"769\"\n\t{\n\t\t\"official\"\t\t\"desktop.vdf\"\n\t}\n";
const BLACKLIST_KEY: &str = "\"controller_blacklist\"";
const PUCK_BLACKLIST: &[&str] = &["28de/1302", "28de/1303", "28de/1304", "28de/1305"];
const BLACKLIST_LINE: &str =
    "\t\"controller_blacklist\"\t\t\"28de/1302,28de/1303,28de/1304,28de/1305\"\n";

#[must_use]
pub fn vdf_has_empty_769(body: &str) -> bool {
    body.contains("\"769\"") && body.contains("empty.vdf")
}

#[must_use]
pub fn vdf_has_desktop_769(body: &str) -> bool {
    body.contains("\"769\"") && body.contains("desktop.vdf") && !body.contains("empty.vdf")
}

#[must_use]
pub fn configset_is_override_target(name: &str) -> bool {
    name == "configset_controller_triton.vdf"
        || name.starts_with("configset_FX")
        || name.starts_with("configset_45e-28e-")
}

fn parse_blacklist_ids(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn is_puck_blacklist_id(id: &str) -> bool {
    PUCK_BLACKLIST
        .iter()
        .any(|puck| puck.eq_ignore_ascii_case(id))
}

fn find_blacklist(body: &str) -> Option<(usize, usize, usize)> {
    let key_start = body.find(BLACKLIST_KEY)?;
    let after_key = key_start + BLACKLIST_KEY.len();
    let rel_quote = body[after_key..].find('"')?;
    let value_start = after_key + rel_quote + 1;
    let value_end = value_start + body[value_start..].find('"')?;
    Some((key_start, value_start, value_end))
}

fn insert_blacklist_key(body: &str) -> String {
    if let Some(idx) = body.rfind('}') {
        let mut out = String::with_capacity(body.len() + BLACKLIST_LINE.len());
        out.push_str(&body[..idx]);
        out.push_str(BLACKLIST_LINE);
        out.push_str(&body[idx..]);
        out
    } else {
        format!("{body}{BLACKLIST_LINE}")
    }
}

/// Add or remove puck VID/PIDs in Steam's `controller_blacklist`.
#[must_use]
pub fn vdf_set_controller_blacklist(body: &str, enable: bool) -> Option<String> {
    let Some((key_start, value_start, value_end)) = find_blacklist(body) else {
        return enable.then(|| insert_blacklist_key(body));
    };
    let current = &body[value_start..value_end];
    let mut ids = parse_blacklist_ids(current);
    let before = ids.clone();
    if enable {
        for puck in PUCK_BLACKLIST {
            if !ids.iter().any(|id| id.eq_ignore_ascii_case(puck)) {
                ids.push((*puck).to_string());
            }
        }
    } else {
        ids.retain(|id| !is_puck_blacklist_id(id));
    }
    if ids == before {
        return None;
    }
    if ids.is_empty() {
        let bytes = body.as_bytes();
        let mut start = key_start;
        while start > 0 && matches!(bytes[start - 1], b' ' | b'\t') {
            start -= 1;
        }
        let mut end = value_end + 1;
        if bytes.get(end) == Some(&b'\n') {
            end += 1;
        }
        let mut out = String::with_capacity(body.len());
        out.push_str(&body[..start]);
        out.push_str(&body[end..]);
        return Some(out);
    }
    let mut out = String::with_capacity(body.len() + 32);
    out.push_str(&body[..value_start]);
    out.push_str(&ids.join(","));
    out.push_str(&body[value_end..]);
    Some(out)
}

fn patch_controller_blacklist_file(path: &Path, enable: bool) {
    let Ok(body) = fs::read_to_string(path) else {
        return;
    };
    let Some(updated) = vdf_set_controller_blacklist(&body, enable) else {
        return;
    };
    if fs::write(path, updated).is_err() {
        logln(format!("could not write {}", path.display()));
        return;
    }
    logln(format!(
        "Steam controller_blacklist {} in {}",
        if enable { "set" } else { "cleared" },
        path.display()
    ));
}

fn steam_config_vdf_paths(home: &Path) -> Vec<std::path::PathBuf> {
    let rels = [
        ".local/share/Steam/config/config.vdf",
        ".steam/steam/config/config.vdf",
        ".steam/root/config/config.vdf",
    ];
    let mut out = Vec::new();
    for rel in rels {
        let path = home.join(rel);
        if !path.is_file() {
            continue;
        }
        if out
            .iter()
            .any(|seen: &std::path::PathBuf| seen.canonicalize().ok() == path.canonicalize().ok())
        {
            continue;
        }
        out.push(path);
    }
    out
}

fn vdf_set_769(body: &str, block: &str, already: impl Fn(&str) -> bool) -> Option<String> {
    if already(body) {
        return None;
    }
    if let Some(start) = body.find("\"769\"") {
        let after = &body[start..];
        let rel_brace = after.find('{')?;
        let brace = start + rel_brace;
        let mut depth = 1;
        let mut end = brace + 1;
        let bytes = body.as_bytes();
        while end < body.len() && depth > 0 {
            match bytes[end] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            end += 1;
        }
        if depth != 0 {
            return None;
        }
        while end < body.len() && (bytes[end] == b'\n' || bytes[end] == b'\r') {
            end += 1;
        }
        let mut out = String::with_capacity(body.len() + block.len());
        out.push_str(&body[..start]);
        out.push_str(block);
        out.push_str(&body[end..]);
        return Some(out);
    }
    let brace = body.find('{')?;
    let mut insert = brace + 1;
    if body.as_bytes().get(insert) == Some(&b'\n') {
        insert += 1;
    }
    let mut out = String::with_capacity(body.len() + block.len());
    out.push_str(&body[..insert]);
    out.push_str(block);
    out.push_str(&body[insert..]);
    Some(out)
}

#[must_use]
pub fn vdf_set_empty_769(body: &str) -> Option<String> {
    vdf_set_769(body, EMPTY_769, vdf_has_empty_769)
}

#[must_use]
pub fn vdf_set_desktop_769(body: &str) -> Option<String> {
    vdf_set_769(body, DESKTOP_769, vdf_has_desktop_769)
}

fn copy_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    paths::ensure_parent(dst);
    fs::copy(src, dst).map(|_| ())
}

fn patch_configset_file(path: &Path, enable: bool) {
    let Some(base) = path.file_name().and_then(|n| n.to_str()) else {
        return;
    };
    let bak = cfgbak_dir().join(base);
    if !enable {
        if bak.is_file() && copy_file(&bak, path).is_ok() {
            let _ = fs::remove_file(&bak);
            logln(format!("restored Steam config {}", path.display()));
        }
        let Ok(body) = fs::read_to_string(path) else {
            return;
        };
        let Some(updated) = vdf_set_desktop_769(&body) else {
            return;
        };
        if fs::write(path, updated).is_ok() {
            logln(format!(
                "Steam desktop layout restored in {}",
                path.display()
            ));
        }
        return;
    }
    let Ok(body) = fs::read_to_string(path) else {
        return;
    };
    if vdf_has_empty_769(&body) {
        return;
    }
    if !bak.exists() && copy_file(path, &bak).is_err() {
        logln(format!("could not backup {}", path.display()));
    }
    let Some(updated) = vdf_set_empty_769(&body) else {
        return;
    };
    if fs::write(path, updated).is_err() {
        logln(format!("could not write {}", path.display()));
        return;
    }
    logln(format!(
        "Steam desktop layout emptied in {}",
        path.display()
    ));
}

pub fn hide_steam_desktop_config(enable: bool) {
    let Some(home) = std::env::var_os("HOME").filter(|v| !v.is_empty()) else {
        return;
    };
    let home = Path::new(&home);
    for path in steam_config_vdf_paths(home) {
        patch_controller_blacklist_file(&path, enable);
    }
    let roots = [
        ".local/share/Steam/steamapps/common/Steam Controller Configs",
        ".steam/steam/steamapps/common/Steam Controller Configs",
        ".steam/root/steamapps/common/Steam Controller Configs",
    ];
    for rel in roots {
        let root = home.join(rel);
        let Ok(accounts) = fs::read_dir(&root) else {
            continue;
        };
        for account in accounts.flatten() {
            if account.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let cfgdir = account.path().join("config");
            let Ok(files) = fs::read_dir(&cfgdir) else {
                continue;
            };
            for file in files.flatten() {
                let name = file.file_name();
                let Some(name) = name.to_str() else {
                    continue;
                };
                if configset_is_override_target(name) {
                    patch_configset_file(&file.path(), enable);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_769_after_root_brace() {
        let src = "\"controller_generic\"\n{\n\t\"1\"\n\t{\n\t}\n}\n";
        let out = vdf_set_empty_769(src).expect("patch");
        assert!(vdf_has_empty_769(&out));
        assert!(out.contains("empty.vdf"));
    }

    #[test]
    fn replaces_existing_769() {
        let src = "{\n\t\"769\"\n\t{\n\t\t\"official\"\t\t\"desktop.vdf\"\n\t}\n}\n";
        let out = vdf_set_empty_769(src).expect("patch");
        assert!(vdf_has_empty_769(&out));
        assert!(!out.contains("desktop.vdf"));
    }

    #[test]
    fn already_empty_is_noop() {
        let src = "{\n\t\"769\"\n\t{\n\t\t\"official\"\t\t\"empty.vdf\"\n\t}\n}\n";
        assert!(vdf_set_empty_769(src).is_none());
        let desk = vdf_set_desktop_769(src).expect("desktop");
        assert!(vdf_has_desktop_769(&desk));
        assert!(!vdf_has_empty_769(&desk));
        assert!(vdf_set_desktop_769(&desk).is_none());
        let empty_root = "\"controller_config\"\n{\n}\n";
        let inserted = vdf_set_desktop_769(empty_root).expect("insert desktop");
        assert!(vdf_has_desktop_769(&inserted));
    }

    #[test]
    fn target_names() {
        assert!(configset_is_override_target(
            "configset_controller_triton.vdf"
        ));
        assert!(configset_is_override_target("configset_FX123.vdf"));
        assert!(configset_is_override_target("configset_45e-28e-0110.vdf"));
        assert!(!configset_is_override_target("configset_other.vdf"));
    }

    #[test]
    fn unmatched_brace_and_missing_root() {
        assert!(vdf_set_empty_769("\"769\" {").is_none());
        assert!(vdf_set_empty_769("no braces here").is_none());
        let nested = "{\n\t\"769\"\n\t{\n\t\t\"inner\"\n\t\t{\n\t\t}\n\t}\n}\n";
        let out = vdf_set_empty_769(nested).expect("replace nested");
        assert!(vdf_has_empty_769(&out));
    }

    #[test]
    fn hide_and_restore_steam_config() {
        crate::test_env::isolated(|root| {
            let cfg = root.join(
                "home/.local/share/Steam/steamapps/common/Steam Controller Configs/42/config",
            );
            std::fs::create_dir_all(&cfg).unwrap();
            std::fs::write(
                cfg.join("configset_controller_triton.vdf"),
                "{\n\t\"1\"\n\t{\n\t}\n}\n",
            )
            .unwrap();
            std::fs::write(cfg.join("configset_other.vdf"), "{}\n").unwrap();
            std::fs::create_dir_all(
                root.join("home/.local/share/Steam/steamapps/common/Steam Controller Configs/.dot"),
            )
            .unwrap();
            let vdf_dir = root.join("home/.local/share/Steam/config");
            std::fs::create_dir_all(&vdf_dir).unwrap();
            std::fs::write(
                vdf_dir.join("config.vdf"),
                "\"InstallConfigStore\"\n{\n\t\"Software\"\n\t{\n\t}\n}\n",
            )
            .unwrap();
            hide_steam_desktop_config(true);
            let body =
                std::fs::read_to_string(cfg.join("configset_controller_triton.vdf")).unwrap();
            assert!(vdf_has_empty_769(&body));
            let store = std::fs::read_to_string(vdf_dir.join("config.vdf")).unwrap();
            assert!(store.contains("28de/1304"));
            hide_steam_desktop_config(true);
            hide_steam_desktop_config(false);
            let restored =
                std::fs::read_to_string(cfg.join("configset_controller_triton.vdf")).unwrap();
            assert!(!vdf_has_empty_769(&restored));
            assert!(vdf_has_desktop_769(&restored));
            let store = std::fs::read_to_string(vdf_dir.join("config.vdf")).unwrap();
            assert!(!store.contains("controller_blacklist"));
        });
    }

    #[test]
    fn controller_blacklist_merge_and_remove() {
        let src = "\"InstallConfigStore\"\n{\n\t\"Software\"\n\t{\n\t}\n}\n";
        let added = vdf_set_controller_blacklist(src, true).expect("insert");
        assert!(added.contains("28de/1304"));
        assert!(vdf_set_controller_blacklist(&added, true).is_none());
        let with_other = added.replace(
            "28de/1302,28de/1303,28de/1304,28de/1305",
            "045e/02a1,28de/1304",
        );
        let merged = vdf_set_controller_blacklist(&with_other, true).expect("merge");
        assert!(merged.contains("045e/02a1"));
        assert!(merged.contains("28de/1302"));
        let cleared = vdf_set_controller_blacklist(&merged, false).expect("clear puck");
        assert!(cleared.contains("045e/02a1"));
        assert!(!cleared.contains("28de/1304"));
        let gone = vdf_set_controller_blacklist(&added, false).expect("drop key");
        assert!(!gone.contains("controller_blacklist"));
        assert!(vdf_set_controller_blacklist(src, false).is_none());
    }
}
