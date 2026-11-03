//! Monitor and selection types shared by the overlay and the recorders, and their coordinate maths.

use serde::{Deserialize, Serialize};

/// A monitor in global logical coordinates (points on macOS, DIPs on Windows).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
}

/// A point in monitor-local logical px.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// The cursor (positioning space: points when `points`, else physical px) on monitor `m`, monitor-local;
/// `None` when it is on another monitor.
pub fn local_cursor((cx, cy): (f64, f64), m: &MonitorInfo, points: bool) -> Option<Point> {
    let k = if points { 1.0 } else { m.scale };
    let (x, y) = (cx / k - m.x, cy / k - m.y);
    (x >= 0.0 && y >= 0.0 && x < m.width && y < m.height).then_some(Point { x, y })
}

/// What the user picked in the overlay. A rect is in monitor-local logical px.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Selection {
    Rect { x: f64, y: f64, width: f64, height: f64 },
    Window { id: u64 },
}

/// `(x, y, width, height)`.
pub type RectF = (f64, f64, f64, f64);

/// Monitor-local logical rect → global logical rect.
pub fn to_global(r: RectF, m: &MonitorInfo) -> RectF {
    (m.x + r.0, m.y + r.1, r.2, r.3)
}

/// Monitor-local logical rect → global physical pixels (the global rect times the monitor scale), at least 1×1.
pub fn to_physical(r: RectF, m: &MonitorInfo) -> (i32, i32, u32, u32) {
    let (x, y, w, h) = to_global(r, m);
    let px = |v: f64| (v * m.scale).round();
    (px(x) as i32, px(y) as i32, px(w).max(1.0) as u32, px(h).max(1.0) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cursor_is_local_to_its_monitor_only() {
        let m = MonitorInfo { x: 1440.0, y: 0.0, width: 1920.0, height: 1080.0, scale: 2.0 };
        assert_eq!(local_cursor((1500.0, 30.0), &m, true), Some(Point { x: 60.0, y: 30.0 }));
        assert_eq!(local_cursor((3000.0, 60.0), &m, false), Some(Point { x: 60.0, y: 30.0 }), "physical px on Windows");
        assert_eq!(local_cursor((100.0, 30.0), &m, true), None);
    }

    #[test]
    fn to_physical_scales_and_offsets() {
        let m = MonitorInfo { x: 1440.0, y: 0.0, width: 1920.0, height: 1080.0, scale: 2.0 };
        let r = (10.0, 20.0, 100.0, 50.0);
        assert_eq!(to_global(r, &m), (1450.0, 20.0, 100.0, 50.0));
        assert_eq!(to_physical(r, &m), (2900, 40, 200, 100));
    }
}
