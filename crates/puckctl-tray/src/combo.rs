use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk4::cairo::{Context, LinearGradient};
use gtk4::glib::{self, ControlFlow, ExitCode};
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, DrawingArea, Orientation, Overlay};
use puckctl_protocol::{format_buttons, is_settable_combo, sanitize_buttons};

const HOLD: Duration = Duration::from_secs(5);
const APP_ID: &str = "dev.puckctl.combo";
const INSTRUCTIONS: &str = "Hold at least two buttons for 5 seconds without adding or releasing any. The fill sweeps left to right, and any change starts over.";

#[derive(Debug, Clone, Copy)]
enum Finish {
    Set,
    Cleared,
}

struct Capture {
    held: u32,
    since: Option<Instant>,
    saved: u32,
    finish: Option<Finish>,
}

impl Capture {
    fn new(saved: u32) -> Self {
        Self {
            held: 0,
            since: None,
            saved,
            finish: None,
        }
    }

    fn progress(&self) -> f64 {
        if self.finish.is_some() {
            return 1.0;
        }
        if !is_settable_combo(self.held) {
            return 0.0;
        }
        let Some(since) = self.since else {
            return 0.0;
        };
        (since.elapsed().as_secs_f64() / HOLD.as_secs_f64()).clamp(0.0, 1.0)
    }

    fn apply_buttons(&mut self, buttons: u32, combo: u32) -> bool {
        if self.finish.is_some() {
            return false;
        }
        if buttons != self.held {
            self.held = buttons;
            self.since = is_settable_combo(buttons).then(Instant::now);
        }
        self.saved = combo;
        if self.progress() >= 1.0 && is_settable_combo(self.held) {
            self.saved = self.held;
            self.finish = Some(Finish::Set);
            return true;
        }
        false
    }

    fn mark_cleared(&mut self) {
        self.saved = 0;
        self.finish = Some(Finish::Cleared);
    }
}

#[derive(Clone)]
struct Labels {
    title: gtk4::Label,
    help: gtk4::Label,
    input: gtk4::Label,
    hint: gtk4::Label,
}

#[must_use]
pub fn parse_hex_field(text: &str, key: &str) -> u32 {
    for part in text.replace("OK ", "").split_whitespace() {
        if let Some((k, value)) = part.split_once('=')
            && k == key
        {
            return u32::from_str_radix(value.trim_end_matches('\n'), 16).unwrap_or(0);
        }
    }
    0
}

fn which_puckctl() -> PathBuf {
    if let Some(env) = std::env::var_os("PUCKCTL") {
        return PathBuf::from(env);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let sibling = dir.join("puckctl");
        if sibling.is_file() {
            return sibling;
        }
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

fn notify(summary: &str, body: &str) {
    let _ = Command::new("notify-send")
        .args(["-a", "puckctl", "--", summary, body])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[allow(clippy::cast_precision_loss)]
fn paint_sweep(cr: &Context, width: i32, height: i32, progress: f64, success: bool) {
    if progress <= 0.0 || width <= 0 || height <= 0 {
        return;
    }
    let w = f64::from(width);
    let h = f64::from(height);
    cr.rectangle(0.0, 0.0, (w * progress).max(1.0), h);
    cr.clip();
    let grad = LinearGradient::new(0.0, 0.0, w, 0.0);
    if success {
        grad.add_color_stop_rgba(0.0, 0.18, 0.72, 0.38, 0.10);
        grad.add_color_stop_rgba(1.0, 0.22, 0.82, 0.45, 0.48);
    } else {
        grad.add_color_stop_rgba(0.0, 0.25, 0.52, 0.95, 0.10);
        grad.add_color_stop_rgba(1.0, 0.40, 0.68, 1.0, 0.50);
    }
    let _ = cr.set_source(&grad);
    let _ = cr.paint();
}

fn apply_css() {
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(
        "
        .combo-title { font-weight: 700; font-size: 16pt; }
        .combo-help { font-size: 11pt; opacity: 0.85; }
        .combo-input { font-weight: 600; font-size: 22pt; }
        .combo-hint { font-size: 10pt; opacity: 0.7; }
        .combo-content { background: transparent; }
        ",
    );
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn input_text(mask: u32) -> String {
    let names = format_buttons(mask);
    if names.is_empty() {
        "Waiting for buttons…".into()
    } else {
        names
    }
}

fn current_hint(saved: u32) -> String {
    if saved == 0 {
        "Current combo: not set".into()
    } else {
        format!("Current combo: {}", format_buttons(saved))
    }
}

fn labeled(text: &str, class: &str) -> gtk4::Label {
    let label = gtk4::Label::new(Some(text));
    label.add_css_class(class);
    label.set_halign(gtk4::Align::Start);
    label.set_wrap(true);
    label
}

fn close_soon(window: &ApplicationWindow, ms: u64) {
    let win = window.clone();
    glib::timeout_add_local_once(Duration::from_millis(ms), move || win.close());
}

fn wire_clear(
    clear: &gtk4::Button,
    state: &Rc<RefCell<Capture>>,
    labels: &Labels,
    window: &ApplicationWindow,
    progress: &Rc<Cell<f64>>,
    sweep: &DrawingArea,
) {
    let state = Rc::clone(state);
    let labels = labels.clone();
    let window = window.clone();
    let progress = Rc::clone(progress);
    let sweep = sweep.clone();
    clear.connect_clicked(move |btn| {
        let _ = run_puckctl(&["combo", "clear"]);
        notify("puckctl", "Desktop combo cleared");
        let mut cap = state.borrow_mut();
        cap.mark_cleared();
        progress.set(1.0);
        drop(cap);
        labels.title.set_text("Combo cleared");
        labels
            .help
            .set_text("The gamepad-to-desktop shortcut is off.");
        labels.input.set_text("Cleared");
        labels.hint.set_text(&current_hint(0));
        btn.set_sensitive(false);
        sweep.queue_draw();
        close_soon(&window, 1400);
    });
}

fn tick_capture(
    state: &Rc<RefCell<Capture>>,
    labels: &Labels,
    clear: &gtk4::Button,
    window: &ApplicationWindow,
    progress: &Rc<Cell<f64>>,
    sweep: &DrawingArea,
) -> ControlFlow {
    let mut cap = state.borrow_mut();
    if cap.finish.is_some() {
        let _ = run_puckctl(&["buttons"]);
        return ControlFlow::Continue;
    }
    let reply = run_puckctl(&["buttons"]);
    let buttons = sanitize_buttons(parse_hex_field(&reply, "buttons"));
    let combo = parse_hex_field(&reply, "combo");
    let just_set = cap.apply_buttons(buttons, combo);
    let p = cap.progress();
    progress.set(p);
    labels.input.set_text(&input_text(buttons));
    labels.hint.set_text(&current_hint(cap.saved));
    clear.set_sensitive(cap.saved != 0);
    if just_set {
        let mask = cap.held;
        let _ = run_puckctl(&["combo", &format!("0x{mask:x}")]);
        notify(
            "puckctl",
            &format!("Desktop combo set to {}", format_buttons(mask)),
        );
        labels.title.set_text("Combo set");
        labels.help.set_text(&format!(
            "Hold {} in gamepad mode to switch to keyboard and mouse.",
            format_buttons(mask)
        ));
        labels.input.set_text(&format_buttons(mask));
        labels.hint.set_text(&current_hint(mask));
        clear.set_sensitive(true);
        close_soon(window, 1600);
    }
    drop(cap);
    sweep.queue_draw();
    ControlFlow::Continue
}

fn build_ui(app: &Application) {
    apply_css();
    let saved = parse_hex_field(&run_puckctl(&["combo"]), "combo");
    let state = Rc::new(RefCell::new(Capture::new(saved)));
    let progress = Rc::new(Cell::new(0.0_f64));

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Set desktop combo")
        .default_width(520)
        .default_height(260)
        .build();

    let sweep = DrawingArea::new();
    sweep.set_hexpand(true);
    sweep.set_vexpand(true);
    let progress_draw = Rc::clone(&progress);
    let state_draw = Rc::clone(&state);
    sweep.set_draw_func(move |_, cr, width, height| {
        let success = matches!(state_draw.borrow().finish, Some(Finish::Set));
        paint_sweep(cr, width, height, progress_draw.get(), success);
    });

    let labels = Labels {
        title: labeled("Set desktop combo", "combo-title"),
        help: labeled(INSTRUCTIONS, "combo-help"),
        input: labeled(&input_text(0), "combo-input"),
        hint: labeled(&current_hint(saved), "combo-hint"),
    };
    labels.help.set_xalign(0.0);

    let clear = gtk4::Button::with_label("Clear combo");
    clear.set_halign(gtk4::Align::Start);
    clear.set_sensitive(saved != 0);

    let content = gtk4::Box::new(Orientation::Vertical, 10);
    content.add_css_class("combo-content");
    content.set_margin_top(18);
    content.set_margin_bottom(18);
    content.set_margin_start(20);
    content.set_margin_end(20);
    content.append(&labels.title);
    content.append(&labels.help);
    content.append(&labels.input);
    content.append(&labels.hint);
    content.append(&clear);

    let overlay = Overlay::new();
    overlay.set_child(Some(&sweep));
    overlay.add_overlay(&content);
    window.set_child(Some(&overlay));

    wire_clear(&clear, &state, &labels, &window, &progress, &sweep);

    let state_tick = Rc::clone(&state);
    let progress_tick = Rc::clone(&progress);
    let labels_tick = labels.clone();
    let clear_tick = clear.clone();
    let window_tick = window.clone();
    let sweep_tick = sweep.clone();
    glib::timeout_add_local(Duration::from_millis(50), move || {
        tick_capture(
            &state_tick,
            &labels_tick,
            &clear_tick,
            &window_tick,
            &progress_tick,
            &sweep_tick,
        )
    });

    window.present();
}

pub fn run() -> ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    let argv = [std::env::args()
        .next()
        .unwrap_or_else(|| "puckctl-tray".into())];
    app.run_with_args(&argv)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_button_hex() {
        assert_eq!(
            parse_hex_field("OK buttons=11 connected=1 combo=0", "buttons"),
            0x11
        );
        assert_eq!(parse_hex_field("OK combo=2a", "combo"), 0x2a);
        assert_eq!(parse_hex_field("nope", "combo"), 0);
        assert_eq!(parse_hex_field("OK combo=zz", "combo"), 0);
    }

    #[test]
    fn input_and_hint_text() {
        assert_eq!(input_text(0), "Waiting for buttons…");
        assert!(input_text(puckctl_protocol::BTN_A).contains('A'));
        assert_eq!(current_hint(0), "Current combo: not set");
        assert!(current_hint(puckctl_protocol::BTN_A | puckctl_protocol::BTN_STEAM).contains('A'));
    }

    #[test]
    fn capture_hold_and_clear() {
        let combo = puckctl_protocol::BTN_A | puckctl_protocol::BTN_STEAM;
        let mut cap = Capture::new(0);
        assert!(cap.progress() < 0.01);
        assert!(!cap.apply_buttons(puckctl_protocol::BTN_A, 0));
        assert!(cap.progress() < 0.01);
        assert!(!cap.apply_buttons(combo, 0x11));
        assert!(cap.progress() < 1.0);
        cap.since = Instant::now().checked_sub(HOLD);
        assert!(cap.apply_buttons(combo, 0x11));
        assert!(matches!(cap.finish, Some(Finish::Set)));
        assert!(cap.progress() > 0.99);
        assert!(!cap.apply_buttons(0, 0));
        cap.mark_cleared();
        assert!(matches!(cap.finish, Some(Finish::Cleared)));
        assert_eq!(cap.saved, 0);
    }

    #[test]
    fn which_puckctl_has_a_name() {
        assert!(which_puckctl().ends_with("puckctl"));
        let _ = run_puckctl(&["status"]);
        notify("puckctl-test", "coverage");
    }

    #[test]
    fn paint_sweep_on_image_surface() {
        let surface = gtk4::cairo::ImageSurface::create(gtk4::cairo::Format::ARgb32, 80, 20)
            .expect("cairo surface");
        let cr = Context::new(&surface).expect("cairo");
        paint_sweep(&cr, 80, 20, 0.0, false);
        paint_sweep(&cr, 0, 20, 0.5, false);
        paint_sweep(&cr, 80, 20, 0.4, false);
        paint_sweep(&cr, 80, 20, 1.0, true);
    }

    #[test]
    fn combo_window_builds_when_display_exists() {
        static GTK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _lock = GTK.lock().expect("gtk");
        if gtk4::init().is_err() {
            return;
        }
        let app = Application::builder()
            .application_id("dev.puckctl.combo.test")
            .build();
        app.connect_activate(|app| {
            build_ui(app);
            let app = app.clone();
            glib::timeout_add_local_once(Duration::from_millis(80), move || app.quit());
        });
        let argv = [String::from("puckctl-tray-test")];
        let _ = app.run_with_args(&argv);
    }
}
