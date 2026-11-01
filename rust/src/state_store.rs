//! Persisted color state.
//!
//! The framework only pushes setFeature once during display service init (and on
//! user changes); it does not replay on HAL restart. Persisting the last applied
//! state lets the HAL restore it right after boot even if the client's initial
//! push is lost to a bind/registration race.

use crate::color::State;
use std::sync::{Mutex, OnceLock};

const DIR: &str = "/data/vendor/displayfeature";
const PATH: &str = "/data/vendor/displayfeature/state";
const TMP: &str = "/data/vendor/displayfeature/state.tmp";

static LAST: OnceLock<Mutex<Option<String>>> = OnceLock::new();

pub fn load() -> Option<State> {
    let text = std::fs::read_to_string(PATH).ok()?;
    let mut it = text.split_whitespace();
    let ver = it.next()?;
    if ver != "v1" {
        return None;
    }
    let style: i32 = it.next()?.parse().ok()?;
    let temperature: i32 = it.next()?.parse().ok()?;
    let eye: i32 = it.next()?.parse().ok()?;
    let has_gains: i32 = it.next()?.parse().ok()?;
    let mut color_gains = None;
    if has_gains == 1 {
        let r: f32 = it.next()?.parse().ok()?;
        let g: f32 = it.next()?.parse().ok()?;
        let b: f32 = it.next()?.parse().ok()?;
        if !(r.is_finite() && g.is_finite() && b.is_finite()) || r <= 0.0 || g <= 0.0 || b <= 0.0 {
            return None;
        }
        color_gains = Some([r, g, b]);
    }
    if !(0..=2).contains(&style) || !(1..=3).contains(&temperature) || !(0..=255).contains(&eye) {
        return None;
    }
    Some(State {
        style,
        temperature,
        eye,
        color_gains,
    })
}

pub fn save(s: &State) -> bool {
    let gains = match s.color_gains {
        Some(v) => format!("1 {:.6} {:.6} {:.6}", v[0], v[1], v[2]),
        None => "0".to_string(),
    };
    let text = format!("v1 {} {} {} {}\n", s.style, s.temperature, s.eye, gains);
    let cache = LAST.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if guard.as_deref() == Some(text.as_str()) {
            return true;
        }
    }
    let _ = std::fs::create_dir_all(DIR);
    if std::fs::write(TMP, text.as_bytes()).is_err() {
        return false;
    }
    if std::fs::rename(TMP, PATH).is_err() {
        return false;
    }
    if let Ok(mut guard) = cache.lock() {
        *guard = Some(text);
    }
    true
}
