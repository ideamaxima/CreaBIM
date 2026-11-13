//! A 3D mouse (3Dconnexion SpaceMouse) read from the spacenavd daemon's
//! socket, in its original protocol: eight 32-bit integers per event.
//! Motion is `0, x, y, z, rx, ry, rz, period`; a button is `1` (press) or
//! `2` (release) and its number. Without spacenavd nothing happens.

use std::sync::{Mutex, OnceLock};

/// What one of the cap's six motions does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Action {
    Off,
    PanX,
    PanY,
    Zoom,
    OrbitX,
    OrbitY,
}

impl Action {
    pub const ALL: [Action; 6] = [
        Action::Off,
        Action::PanX,
        Action::PanY,
        Action::Zoom,
        Action::OrbitX,
        Action::OrbitY,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Action::Off => "Off",
            Action::PanX => "Pan X",
            Action::PanY => "Pan Y",
            Action::Zoom => "Zoom",
            Action::OrbitX => "Orbit X",
            Action::OrbitY => "Orbit Y",
        }
    }
}

/// One motion of the cap: what it does, reversed or not, and how much.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Axis {
    pub action: Action,
    pub invert: bool,
    /// Percent.
    pub speed: f32,
}

/// The cap's six motions, in spacenavd's order.
pub const AXIS_NAMES: [&str; 6] = [
    "Slide left / right",
    "Slide up / down",
    "Push / pull",
    "Tilt forward / back",
    "Turn left / right",
    "Roll left / right",
];

/// The 3D mouse settings, kept with the others.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    /// Percent, for every motion.
    pub speed: f32,
    /// Pan and zoom only, never orbit (to draw in a 3D view seen from above).
    pub no_rotation: bool,
    /// Percent of a full deflection ignored round the rest position, so a
    /// cap left alone that doesn't quite read zero moves nothing.
    pub dead_zone: f32,
    pub axes: [Axis; 6],
}

impl Default for Config {
    fn default() -> Self {
        let a = |action, invert| Axis {
            action,
            invert,
            speed: 100.0,
        };
        Self {
            enabled: true,
            speed: 100.0,
            no_rotation: false,
            dead_zone: 8.0,
            axes: [
                a(Action::PanX, false),
                a(Action::PanY, false),
                a(Action::Zoom, true),
                a(Action::OrbitY, true),
                a(Action::OrbitX, false),
                a(Action::Off, false),
            ],
        }
    }
}

/// The motion asked for, each from -1 to 1 at full deflection.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Motion {
    pub pan: [f32; 2],
    pub zoom: f32,
    pub orbit: [f32; 2],
}

impl Motion {
    pub fn is_zero(&self) -> bool {
        *self == Motion::default()
    }
}

/// About a full deflection of the cap, in spacenavd's units.
const FULL: f32 = 350.0;

/// The raw axes turned into a motion by the settings.
pub fn motion(raw: [i32; 6], cfg: &Config) -> Motion {
    let mut m = Motion::default();
    if !cfg.enabled {
        return m;
    }
    for (v, axis) in raw.iter().zip(&cfg.axes) {
        let dead = (cfg.dead_zone / 100.0).clamp(0.0, 0.9);
        let v = (*v as f32 / FULL).clamp(-1.0, 1.0);
        if v.abs() <= dead {
            continue;
        }
        // past the dead zone the motion starts again from nothing
        let v = v.signum() * (v.abs() - dead) / (1.0 - dead);
        let mut v = v * axis.speed / 100.0 * cfg.speed / 100.0;
        if axis.invert {
            v = -v;
        }
        match axis.action {
            Action::Off => {}
            Action::PanX => m.pan[0] += v,
            Action::PanY => m.pan[1] += v,
            Action::Zoom => m.zoom += v,
            Action::OrbitX if !cfg.no_rotation => m.orbit[0] += v,
            Action::OrbitY if !cfg.no_rotation => m.orbit[1] += v,
            Action::OrbitX | Action::OrbitY => {}
        }
    }
    m
}

#[derive(Default)]
struct Shared {
    connected: bool,
    raw: [i32; 6],
    /// Buttons pressed and not yet taken, by number.
    pressed: Vec<i32>,
    cfg: Config,
}

fn shared() -> &'static Mutex<Shared> {
    static S: OnceLock<Mutex<Shared>> = OnceLock::new();
    S.get_or_init(Default::default)
}

/// Use these settings from now on.
pub fn set_config(cfg: &Config) {
    if let Ok(mut s) = shared().lock() {
        s.cfg = cfg.clone();
    }
}

/// Whether spacenavd answers.
pub fn connected() -> bool {
    shared().lock().map(|s| s.connected).unwrap_or(false)
}

/// The cap as it is now, raw, for the settings' live reading.
pub fn raw() -> [i32; 6] {
    shared().lock().map(|s| s.raw).unwrap_or_default()
}

/// The motion asked for now.
pub fn current() -> Motion {
    shared()
        .lock()
        .map(|s| motion(s.raw, &s.cfg))
        .unwrap_or_default()
}

/// Whether button `n` was pressed since last asked (0 is the left one).
pub fn take_press(n: i32) -> bool {
    let Ok(mut s) = shared().lock() else {
        return false;
    };
    let had = s.pressed.contains(&n);
    s.pressed.retain(|b| *b != n);
    had && s.cfg.enabled
}

/// Start listening to spacenavd, once; each event repaints the window.
#[cfg(unix)]
pub fn start(ctx: &eframe::egui::Context) {
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_err() {
        return;
    }
    let ctx = ctx.clone();
    let _ = std::thread::Builder::new()
        .name("spacenavd".into())
        .spawn(move || listen(ctx));
}

#[cfg(not(unix))]
pub fn start(_ctx: &eframe::egui::Context) {}

#[cfg(unix)]
fn listen(ctx: eframe::egui::Context) {
    use std::io::Read;
    let path = std::env::var("SPNAV_SOCKET").unwrap_or_else(|_| "/var/run/spnav.sock".to_string());
    loop {
        if let Ok(mut stream) = std::os::unix::net::UnixStream::connect(&path) {
            set_connected(true);
            ctx.request_repaint();
            let mut buf = [0u8; 32];
            while stream.read_exact(&mut buf).is_ok() {
                let mut ev = [0i32; 8];
                for (i, v) in ev.iter_mut().enumerate() {
                    *v = i32::from_ne_bytes(buf[i * 4..i * 4 + 4].try_into().unwrap_or_default());
                }
                apply(ev);
                ctx.request_repaint();
            }
            set_connected(false);
            ctx.request_repaint();
        }
        // spacenavd not running (yet): look again now and then
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
}

#[cfg(unix)]
fn set_connected(on: bool) {
    if let Ok(mut s) = shared().lock() {
        s.connected = on;
        s.raw = [0; 6];
        s.pressed.clear();
    }
}

/// One event from spacenavd.
fn apply(ev: [i32; 8]) {
    let Ok(mut s) = shared().lock() else {
        return;
    };
    match ev[0] {
        0 => s.raw.copy_from_slice(&ev[1..7]),
        1 => s.pressed.push(ev[1]),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_axes_pan_zoom_and_orbit() {
        let cfg = Config {
            dead_zone: 0.0,
            ..Config::default()
        };
        let m = motion([350, -175, 350, 350, -350, 350], &cfg);
        assert_eq!(m.pan, [1.0, -0.5]);
        // push/pull and tilt are reversed; roll does nothing
        assert_eq!(m.zoom, -1.0);
        assert_eq!(m.orbit, [-1.0, -1.0]);
    }

    #[test]
    fn a_cap_barely_off_rest_moves_nothing() {
        let cfg = Config::default();
        assert!(motion([20, -20, 25, 15, -25, 0], &cfg).is_zero());
        // and past the dead zone it starts gently
        let m = motion([50, 0, 0, 0, 0, 0], &cfg);
        assert!(m.pan[0] > 0.0 && m.pan[0] < 0.1);
    }

    #[test]
    fn pan_and_zoom_only_leaves_orbit_out() {
        let cfg = Config {
            no_rotation: true,
            dead_zone: 0.0,
            ..Config::default()
        };
        let m = motion([0, 0, 0, 350, 350, 0], &cfg);
        assert!(m.is_zero());
    }

    #[test]
    fn speeds_multiply_and_deflection_is_capped() {
        let mut cfg = Config {
            speed: 200.0,
            ..Config::default()
        };
        cfg.axes[0].speed = 50.0;
        let m = motion([700, 0, 0, 0, 0, 0], &cfg);
        assert_eq!(m.pan[0], 1.0);
    }

    #[test]
    fn switched_off_it_does_nothing() {
        let cfg = Config {
            enabled: false,
            ..Config::default()
        };
        assert!(motion([350; 6], &cfg).is_zero());
    }

    #[test]
    fn events_set_the_axes_and_buttons() {
        apply([0, 1, 2, 3, 4, 5, 6, 16]);
        assert_eq!(raw(), [1, 2, 3, 4, 5, 6]);
        apply([1, 0, 0, 0, 0, 0, 0, 0]);
        assert!(take_press(0));
        assert!(!take_press(0));
        apply([0; 8]);
    }
}
