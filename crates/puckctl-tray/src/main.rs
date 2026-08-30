mod combo;
mod icons;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use ksni::menu::{CheckmarkItem, RadioGroup, RadioItem, StandardItem};
use ksni::{Handle, MenuItem, ToolTip, Tray, TrayMethods};
use puckctl_protocol::{VERSION, format_buttons};

const APP_TITLE: &str = "puckctl";
const DAEMON_UNIT: &str = "puckctl.service";
const TRAY_UNIT: &str = "puckctl-tray.service";

#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
struct Status {
    effective: String,
    requested: String,
    steam: bool,
    override_steam: bool,
    connected: bool,
    combo: u32,
    daemon: bool,
}

impl Default for Status {
    fn default() -> Self {
        Self {
            effective: "lizard".into(),
            requested: "unknown".into(),
            steam: false,
            override_steam: false,
            connected: false,
            combo: 0,
            daemon: false,
        }
    }
}

fn parse_status(text: &str) -> Status {
    let mut status = Status::default();
    for part in text.replace("OK ", "").split_whitespace() {
        if let Some((key, value)) = part.split_once('=') {
            match key {
                "effective" => status.effective = value.to_string(),
                "requested" => status.requested = value.to_string(),
                "steam" => status.steam = value == "1",
                "override" => status.override_steam = value == "1",
                "connected" => status.connected = value == "1",
                "combo" => status.combo = u32::from_str_radix(value, 16).unwrap_or(0),
                "daemon" => status.daemon = value == "1",
                _ => {}
            }
        }
    }
    status
}

fn which_puckctl_from(env: Option<PathBuf>, sibling_dir: Option<&Path>) -> PathBuf {
    if let Some(env) = env {
        return env;
    }
    if let Some(dir) = sibling_dir {
        let sibling = dir.join("puckctl");
        if sibling.is_file() {
            return sibling;
        }
    }
    PathBuf::from("puckctl")
}

fn which_puckctl() -> PathBuf {
    if let Some(env) = std::env::var_os("PUCKCTL") {
        return PathBuf::from(env);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        return which_puckctl_from(None, Some(dir));
    }
    PathBuf::from("puckctl")
}

fn run_puckctl(args: &[&str]) -> String {
    Command::new(which_puckctl())
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn userctl(args: &[&str]) -> bool {
    Command::new("systemctl")
        .arg("--user")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn unit_enabled(unit: &str) -> bool {
    userctl(&["is-enabled", "--quiet", unit])
}

struct PuckTray {
    status: Status,
    login: bool,
    exit: bool,
}

impl PuckTray {
    fn new() -> Self {
        let mut tray = Self {
            status: Status::default(),
            login: unit_enabled(DAEMON_UNIT),
            exit: false,
        };
        tray.refresh();
        tray
    }

    fn refresh(&mut self) {
        self.status = parse_status(&run_puckctl(&["status"]));
        self.login = unit_enabled(DAEMON_UNIT);
    }

    fn yielded(&self) -> bool {
        self.status.steam && !self.status.override_steam
    }

    fn icon_kind(&self) -> icons::Kind {
        let mode = if self.yielded() {
            match self.status.requested.as_str() {
                "gamepad" | "lizard" => self.status.requested.as_str(),
                _ => self.status.effective.as_str(),
            }
        } else {
            self.status.effective.as_str()
        };
        if mode == "gamepad" {
            icons::Kind::Gamepad
        } else {
            icons::Kind::Desktop
        }
    }

    fn label(&self) -> String {
        if self.yielded() {
            "Steam has the controller".into()
        } else if self.status.effective == "gamepad" {
            if self.status.steam {
                "Gamepad (Steam override)".into()
            } else {
                "Gamepad".into()
            }
        } else {
            "Desktop (lizard)".into()
        }
    }

    fn connection_label(&self) -> &'static str {
        if self.yielded() || self.status.connected {
            "Connected"
        } else {
            "Disconnected"
        }
    }

    fn tooltip_text(&self) -> String {
        let mode = self.label();
        let conn = self.connection_label();
        if mode == conn {
            mode
        } else {
            format!("{mode} — {conn}")
        }
    }

    fn icon_tone(&self) -> icons::Tone {
        if self.yielded() {
            icons::Tone::Steam
        } else if self.status.connected {
            icons::Tone::Color
        } else {
            icons::Tone::Dim
        }
    }

    fn combo_label(&self) -> String {
        if self.status.combo == 0 {
            "Set desktop combo…".into()
        } else {
            format!("Change combo ({})…", format_buttons(self.status.combo))
        }
    }
}

fn spawn_combo_exe(exe: &Path) -> bool {
    Command::new(exe)
        .arg("--set-combo")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}

fn open_combo_window() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = spawn_combo_exe(&exe);
    }
}

fn select_mode(tray: &mut PuckTray, index: usize) {
    let cmd = if index == 0 { "gamepad" } else { "lizard" };
    let _ = run_puckctl(&[cmd]);
    tray.refresh();
}

fn toggle_override(tray: &mut PuckTray) {
    let cmd = if tray.status.override_steam {
        "off"
    } else {
        "on"
    };
    let _ = run_puckctl(&["override", cmd]);
    tray.refresh();
}

fn apply_login(enabled: bool, mut ctl: impl FnMut(&[&str]) -> bool) {
    if enabled {
        let _ = ctl(&["disable", DAEMON_UNIT, TRAY_UNIT]);
    } else {
        let _ = ctl(&["enable", DAEMON_UNIT, TRAY_UNIT]);
        let _ = ctl(&["start", DAEMON_UNIT]);
    }
}

fn request_exit(tray: &mut PuckTray) {
    let _ = run_puckctl(&["quit"]);
    tray.exit = true;
}

fn wants_set_combo<I, S>(args: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter().any(|arg| arg.as_ref() == "--set-combo")
}

fn on_poll(tray: &mut PuckTray) -> bool {
    if tray.exit {
        true
    } else {
        tray.refresh();
        false
    }
}

fn should_stop_poll(result: Option<bool>) -> bool {
    result.unwrap_or(true)
}

async fn run_tray_loop<Fut>(period: Duration, mut poll: impl FnMut() -> Fut)
where
    Fut: std::future::Future<Output = bool>,
{
    let mut interval = tokio::time::interval(period);
    loop {
        interval.tick().await;
        if poll().await {
            break;
        }
    }
}

impl Tray for PuckTray {
    fn id(&self) -> String {
        "puckctl".into()
    }

    fn title(&self) -> String {
        APP_TITLE.into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![icons::icon(self.icon_kind(), self.icon_tone())]
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: APP_TITLE.into(),
            description: self.tooltip_text(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = run_puckctl(&["toggle"]);
        self.refresh();
    }

    fn menu_about_to_show(&mut self) {
        self.refresh();
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let yielded = self.yielded();
        let selected = if yielded {
            usize::from(self.status.requested != "gamepad")
        } else {
            usize::from(self.status.effective != "gamepad")
        };
        vec![
            StandardItem {
                label: format!("{APP_TITLE} {VERSION}"),
                enabled: false,
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: self.connection_label().into(),
                enabled: false,
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            RadioGroup {
                selected,
                select: Box::new(|this: &mut Self, index| select_mode(this, index)),
                options: vec![
                    RadioItem {
                        label: "Gamepad".into(),
                        enabled: !yielded,
                        ..Default::default()
                    },
                    RadioItem {
                        label: "Desktop (keyboard / mouse)".into(),
                        enabled: !yielded,
                        ..Default::default()
                    },
                ],
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: self.combo_label(),
                enabled: self.status.connected && !yielded,
                activate: Box::new(|_this: &mut Self| open_combo_window()),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            CheckmarkItem {
                label: "Override Steam".into(),
                checked: self.status.override_steam,
                activate: Box::new(|this: &mut Self| toggle_override(this)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            CheckmarkItem {
                label: "Start on Login".into(),
                checked: self.login,
                activate: Box::new(|this: &mut Self| {
                    apply_login(this.login, userctl);
                    this.refresh();
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Exit".into(),
                activate: Box::new(|this: &mut Self| request_exit(this)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

fn main() {
    if wants_set_combo(std::env::args()) {
        std::process::exit(i32::from(combo::run()));
    }
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime")
        .block_on(tray_main());
}

fn spawn_failed(err: impl std::fmt::Display) -> String {
    format!("puckctl-tray: {err}")
}

async fn tray_main() {
    let handle: Handle<PuckTray> = match PuckTray::new().spawn().await {
        Ok(handle) => handle,
        Err(err) => {
            eprintln!("{}", spawn_failed(err));
            std::process::exit(1);
        }
    };
    run_tray_loop(Duration::from_secs(2), || async {
        should_stop_poll(handle.update(on_poll).await)
    })
    .await;
    handle.shutdown().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tray(status: Status) -> PuckTray {
        PuckTray {
            status,
            login: false,
            exit: false,
        }
    }

    #[test]
    fn parse_connected_flag() {
        let s = parse_status(
            "OK effective=lizard requested=lizard steam=0 override=1 connected=1 combo=11 daemon=1",
        );
        assert!(s.connected);
        assert_eq!(s.effective, "lizard");
        assert_eq!(s.combo, 0x11);
        assert!(s.override_steam);
        assert!(s.daemon);
        let s = parse_status("OK effective=gamepad requested=gamepad connected=0 daemon=1 extra=1");
        assert!(!s.connected);
        assert_eq!(s.combo, 0);
        assert_eq!(s.effective, "gamepad");
        let d = Status::default();
        assert_eq!(d.effective, "lizard");
        assert!(!d.daemon);
    }

    #[test]
    fn labels_follow_status() {
        let gamepad = Status {
            effective: "gamepad".into(),
            connected: true,
            combo: puckctl_protocol::BTN_A | puckctl_protocol::BTN_STEAM,
            ..Default::default()
        };
        let t = tray(gamepad.clone());
        assert!(!t.yielded());
        assert_eq!(t.icon_kind(), icons::Kind::Gamepad);
        assert_eq!(t.label(), "Gamepad");
        assert_eq!(t.connection_label(), "Connected");
        assert_eq!(t.tooltip_text(), "Gamepad — Connected");
        assert_eq!(t.icon_tone(), icons::Tone::Color);
        assert!(t.combo_label().contains('A'));

        let t = tray(Status {
            steam: true,
            override_steam: true,
            ..gamepad
        });
        assert_eq!(t.label(), "Gamepad (Steam override)");

        let t = tray(Status {
            steam: true,
            effective: "gamepad".into(),
            ..Default::default()
        });
        assert!(t.yielded());
        assert_eq!(t.icon_kind(), icons::Kind::Gamepad);
        assert_eq!(t.label(), "Steam has the controller");
        assert_eq!(t.connection_label(), "Connected");
        assert_eq!(t.tooltip_text(), "Steam has the controller — Connected");
        assert_eq!(t.icon_tone(), icons::Tone::Steam);

        let t = tray(Status::default());
        assert_eq!(t.icon_kind(), icons::Kind::Desktop);
        assert_eq!(t.label(), "Desktop (lizard)");
        assert_eq!(t.connection_label(), "Disconnected");
        assert_eq!(t.icon_tone(), icons::Tone::Dim);
        assert_eq!(t.combo_label(), "Set desktop combo…");
    }

    #[test]
    fn menu_builds_for_yielded_and_live() {
        let t = tray(Status {
            effective: "gamepad".into(),
            connected: true,
            override_steam: true,
            ..Default::default()
        });
        assert_eq!(t.id(), "puckctl");
        assert_eq!(t.title(), APP_TITLE);
        assert_eq!(t.icon_pixmap().len(), 1);
        let tip = t.tool_tip();
        assert!(tip.description.contains("Gamepad"));
        assert_eq!(t.menu().len(), 12);

        let t = tray(Status {
            steam: true,
            requested: "lizard".into(),
            ..Default::default()
        });
        assert_eq!(t.menu().len(), 12);
    }

    #[test]
    fn refresh_and_activate_talk_to_cli() {
        with_stub_cli(|| {
            let mut t = tray(Status::default());
            t.refresh();
            t.menu_about_to_show();
            t.activate(0, 0);
            select_mode(&mut t, 0);
            select_mode(&mut t, 1);
            toggle_override(&mut t);
            request_exit(&mut t);
            assert!(t.exit);
            assert!(on_poll(&mut t));
            t.exit = false;
            assert!(!on_poll(&mut t));
            assert!(!unit_enabled("definitely-not-a-unit.service"));
            let _ = run_puckctl(&["status"]);
            let _ = userctl(&["is-enabled", "--quiet", "puckctl.service"]);
        });
    }

    #[test]
    fn which_puckctl_has_a_name() {
        assert!(which_puckctl().ends_with("puckctl"));
        let env = PathBuf::from("/tmp/custom-puckctl");
        assert_eq!(which_puckctl_from(Some(env.clone()), None), env);
        let dir = std::env::temp_dir().join(format!("puckctl-tray-which-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sibling = dir.join("puckctl");
        std::fs::write(&sibling, b"").unwrap();
        assert_eq!(which_puckctl_from(None, Some(&dir)), sibling);
        std::fs::remove_file(&sibling).unwrap();
        assert_eq!(
            which_puckctl_from(None, Some(&dir)),
            PathBuf::from("puckctl")
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn login_combo_and_loop_helpers() {
        let mut seen = Vec::new();
        apply_login(true, |args| {
            seen.push(args.join(" "));
            true
        });
        apply_login(false, |args| {
            seen.push(args.join(" "));
            true
        });
        assert_eq!(
            seen,
            [
                format!("disable {DAEMON_UNIT} {TRAY_UNIT}"),
                format!("enable {DAEMON_UNIT} {TRAY_UNIT}"),
                format!("start {DAEMON_UNIT}"),
            ]
        );
        assert!(wants_set_combo(["puckctl-tray", "--set-combo"]));
        assert!(!wants_set_combo(["puckctl-tray"]));
        assert!(spawn_combo_exe(Path::new("/bin/true")));
        assert!(!spawn_combo_exe(Path::new("/no/such/puckctl-tray")));
        assert!(should_stop_poll(None));
        assert!(!should_stop_poll(Some(false)));
        assert!(spawn_failed("no bus").contains("puckctl-tray"));
        with_stub_cli(|| {
            let _ = PuckTray::new();
        });
    }

    #[tokio::test]
    async fn tray_loop_stops_on_first_true() {
        let mut ticks = 0u8;
        run_tray_loop(Duration::from_millis(1), || {
            ticks += 1;
            async move { true }
        })
        .await;
        assert_eq!(ticks, 1);
    }

    static CLI_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[allow(unsafe_code)]
    fn with_stub_cli(f: impl FnOnce()) {
        let _guard = CLI_LOCK.lock().expect("cli lock");
        // SAFETY: serialized by CLI_LOCK and restored before unlock.
        unsafe {
            std::env::set_var("PUCKCTL", "/bin/true");
        }
        f();
        unsafe {
            std::env::remove_var("PUCKCTL");
        }
    }
}
