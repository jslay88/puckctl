use std::fs;
use std::path::Path;

use crate::log::logln;
use crate::paths::{self, cfgbak_dir};

const EMPTY_769: &str = "\t\"769\"\n\t{\n\t\t\"official\"\t\t\"empty.vdf\"\n\t}\n";

#[must_use]
pub fn vdf_has_empty_769(body: &str) -> bool {
    body.contains("\"769\"") && body.contains("empty.vdf")
}

#[must_use]
pub fn configset_is_override_target(name: &str) -> bool {
    name == "configset_controller_triton.vdf"
        || name.starts_with("configset_FX")
        || name.starts_with("configset_45e-28e-")
}

#[must_use]
pub fn vdf_set_empty_769(body: &str) -> Option<String> {
    if vdf_has_empty_769(body) {
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
        let mut out = String::with_capacity(body.len() + EMPTY_769.len());
        out.push_str(&body[..start]);
        out.push_str(EMPTY_769);
        out.push_str(&body[end..]);
        return Some(out);
    }
    let brace = body.find('{')?;
    let mut insert = brace + 1;
    if body.as_bytes().get(insert) == Some(&b'\n') {
        insert += 1;
    }
    let mut out = String::with_capacity(body.len() + EMPTY_769.len());
    out.push_str(&body[..insert]);
    out.push_str(EMPTY_769);
    out.push_str(&body[insert..]);
    Some(out)
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
            hide_steam_desktop_config(true);
            let body =
                std::fs::read_to_string(cfg.join("configset_controller_triton.vdf")).unwrap();
            assert!(vdf_has_empty_769(&body));
            hide_steam_desktop_config(true);
            hide_steam_desktop_config(false);
            let restored =
                std::fs::read_to_string(cfg.join("configset_controller_triton.vdf")).unwrap();
            assert!(!vdf_has_empty_769(&restored));
        });
    }
}
