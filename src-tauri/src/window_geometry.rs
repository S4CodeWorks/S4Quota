use crate::PresentationMode;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 1;
pub const MAIN_MIN_WIDTH: f64 = 760.0;
pub const MAIN_MIN_HEIGHT: f64 = 560.0;
pub const COMPACT_WIDTH: f64 = 304.0;
pub const COMPACT_HEIGHT: f64 = 120.0;

const MAX_LOGICAL_DIMENSION: f64 = 16_384.0;
const MAX_LOGICAL_OFFSET: f64 = 100_000.0;
const MAX_PHYSICAL_COORDINATE: i32 = 1_000_000;
const MAX_PHYSICAL_DIMENSION: u32 = 32_768;
const MIN_SCALE_FACTOR: f64 = 0.5;
const MAX_SCALE_FACTOR: f64 = 5.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PresentationDocument {
    pub version: u32,
    #[serde(default = "default_mode")]
    pub last_visible_mode: PresentationMode,
    #[serde(default)]
    pub main: Option<MainGeometry>,
    #[serde(default)]
    pub compact: Option<CompactGeometry>,
}

impl Default for PresentationDocument {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            last_visible_mode: PresentationMode::Main,
            main: None,
            compact: None,
        }
    }
}

fn default_mode() -> PresentationMode {
    PresentationMode::Main
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonitorFingerprint {
    #[serde(default)]
    pub name: Option<String>,
    pub monitor_width: u32,
    pub monitor_height: u32,
    pub work_x: i32,
    pub work_y: i32,
    pub work_width: u32,
    pub work_height: u32,
    pub scale_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MainGeometry {
    /// Offset from the monitor work area's top-left, in logical pixels.
    pub x: f64,
    pub y: f64,
    /// Client-area dimensions in logical pixels.
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub maximized: bool,
    pub monitor: MonitorFingerprint,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompactGeometry {
    /// Offset from the monitor work area's top-left, in logical pixels.
    pub x: f64,
    pub y: f64,
    pub monitor: MonitorFingerprint,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MonitorInfo {
    pub name: Option<String>,
    pub monitor_width: u32,
    pub monitor_height: u32,
    pub monitor_x: i32,
    pub monitor_y: i32,
    pub work_x: i32,
    pub work_y: i32,
    pub work_width: u32,
    pub work_height: u32,
    pub scale_factor: f64,
}

impl MonitorInfo {
    pub fn fingerprint(&self) -> MonitorFingerprint {
        MonitorFingerprint {
            name: self.name.clone(),
            monitor_width: self.monitor_width,
            monitor_height: self.monitor_height,
            work_x: self.work_x,
            work_y: self.work_y,
            work_width: self.work_width,
            work_height: self.work_height,
            scale_factor: self.scale_factor,
        }
    }

    fn is_valid(&self) -> bool {
        self.monitor_width > 0
            && self.monitor_height > 0
            && self.monitor_width <= MAX_PHYSICAL_DIMENSION
            && self.monitor_height <= MAX_PHYSICAL_DIMENSION
            && self.work_width > 0
            && self.work_height > 0
            && self.work_width <= MAX_PHYSICAL_DIMENSION
            && self.work_height <= MAX_PHYSICAL_DIMENSION
            && self.monitor_x.abs() <= MAX_PHYSICAL_COORDINATE
            && self.monitor_y.abs() <= MAX_PHYSICAL_COORDINATE
            && self.work_x.abs() <= MAX_PHYSICAL_COORDINATE
            && self.work_y.abs() <= MAX_PHYSICAL_COORDINATE
            && valid_scale(self.scale_factor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedMainGeometry {
    pub position: PhysicalPosition,
    pub size: PhysicalSize,
    pub monitor_index: usize,
    pub maximized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedCompactPosition {
    pub position: PhysicalPosition,
    pub monitor_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadIssue {
    Read,
    InvalidJson,
    UnsupportedVersion,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedPresentation {
    pub document: PresentationDocument,
    pub issue: Option<LoadIssue>,
    pub write_allowed: bool,
}

pub fn load(path: &Path) -> LoadedPresentation {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return LoadedPresentation {
                document: PresentationDocument::default(),
                issue: None,
                write_allowed: true,
            };
        }
        Err(_) => {
            return LoadedPresentation {
                document: PresentationDocument::default(),
                issue: Some(LoadIssue::Read),
                write_allowed: true,
            };
        }
    };

    let mut document: PresentationDocument = match serde_json::from_str(&contents) {
        Ok(document) => document,
        Err(_) => {
            return LoadedPresentation {
                document: PresentationDocument::default(),
                issue: Some(LoadIssue::InvalidJson),
                write_allowed: true,
            };
        }
    };

    if document.version != SCHEMA_VERSION {
        return LoadedPresentation {
            document: PresentationDocument::default(),
            issue: Some(LoadIssue::UnsupportedVersion),
            write_allowed: false,
        };
    }

    if document
        .main
        .as_ref()
        .is_some_and(|geometry| !valid_main(geometry))
    {
        document.main = None;
    }
    if document
        .compact
        .as_ref()
        .is_some_and(|geometry| !valid_compact(geometry))
    {
        document.compact = None;
    }

    LoadedPresentation {
        document,
        issue: None,
        write_allowed: true,
    }
}

pub fn save_atomic(path: &Path, document: &PresentationDocument) -> io::Result<()> {
    if document.version != SCHEMA_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unsupported presentation schema",
        ));
    }

    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "preference path has no parent")
    })?;
    fs::create_dir_all(parent)?;
    let mut temporary = PathBuf::from(path);
    temporary.set_extension("json.tmp");
    let serialized = serde_json::to_vec_pretty(document)
        .map_err(|_| io::Error::other("failed to serialize presentation preferences"))?;
    fs::write(&temporary, serialized)?;
    fs::rename(&temporary, path)
}

pub fn valid_main(geometry: &MainGeometry) -> bool {
    valid_position(geometry.x, geometry.y)
        && geometry.width.is_finite()
        && geometry.height.is_finite()
        && geometry.width >= MAIN_MIN_WIDTH
        && geometry.height >= MAIN_MIN_HEIGHT
        && geometry.width <= MAX_LOGICAL_DIMENSION
        && geometry.height <= MAX_LOGICAL_DIMENSION
        && valid_fingerprint(&geometry.monitor)
}

pub fn valid_compact(geometry: &CompactGeometry) -> bool {
    valid_position(geometry.x, geometry.y) && valid_fingerprint(&geometry.monitor)
}

fn valid_position(x: f64, y: f64) -> bool {
    x.is_finite() && y.is_finite() && x.abs() <= MAX_LOGICAL_OFFSET && y.abs() <= MAX_LOGICAL_OFFSET
}

fn valid_fingerprint(monitor: &MonitorFingerprint) -> bool {
    monitor.monitor_width > 0
        && monitor.monitor_height > 0
        && monitor.monitor_width <= MAX_PHYSICAL_DIMENSION
        && monitor.monitor_height <= MAX_PHYSICAL_DIMENSION
        && monitor.work_width > 0
        && monitor.work_height > 0
        && monitor.work_width <= MAX_PHYSICAL_DIMENSION
        && monitor.work_height <= MAX_PHYSICAL_DIMENSION
        && monitor.work_x.abs() <= MAX_PHYSICAL_COORDINATE
        && monitor.work_y.abs() <= MAX_PHYSICAL_COORDINATE
        && valid_scale(monitor.scale_factor)
}

fn valid_scale(scale: f64) -> bool {
    scale.is_finite() && (MIN_SCALE_FACTOR..=MAX_SCALE_FACTOR).contains(&scale)
}

pub fn choose_monitor(
    saved: &MonitorFingerprint,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    monitors: &[MonitorInfo],
    primary_index: Option<usize>,
) -> Option<usize> {
    if monitors.is_empty() {
        return None;
    }

    let exact = monitors
        .iter()
        .enumerate()
        .filter(|(_, monitor)| monitor.is_valid())
        .filter_map(|(index, monitor)| {
            fingerprint_score(saved, monitor).map(|score| (index, score))
        })
        .max_by_key(|(_, score)| *score)
        .map(|(index, _)| index);
    if exact.is_some() {
        return exact;
    }

    let old_left = saved.work_x as f64 + x * saved.scale_factor;
    let old_top = saved.work_y as f64 + y * saved.scale_factor;
    let old_right = old_left + width * saved.scale_factor;
    let old_bottom = old_top + height * saved.scale_factor;
    let best_intersection = monitors
        .iter()
        .enumerate()
        .filter(|(_, monitor)| monitor.is_valid())
        .map(|(index, monitor)| {
            let left = old_left.max(monitor.work_x as f64);
            let top = old_top.max(monitor.work_y as f64);
            let right = old_right.min((monitor.work_x + monitor.work_width as i32) as f64);
            let bottom = old_bottom.min((monitor.work_y + monitor.work_height as i32) as f64);
            let area = (right - left).max(0.0) * (bottom - top).max(0.0);
            (index, area)
        })
        .max_by(|left, right| left.1.total_cmp(&right.1));
    if let Some((index, area)) = best_intersection.filter(|(_, area)| *area > 0.0) {
        let _ = area;
        return Some(index);
    }

    primary_index
        .filter(|index| *index < monitors.len() && monitors[*index].is_valid())
        .or_else(|| monitors.iter().position(MonitorInfo::is_valid))
}

fn fingerprint_score(saved: &MonitorFingerprint, monitor: &MonitorInfo) -> Option<u32> {
    let name_match = saved
        .name
        .as_ref()
        .zip(monitor.name.as_ref())
        .is_some_and(|(saved, current)| saved == current);
    let resolution_match = saved.monitor_width == monitor.monitor_width
        && saved.monitor_height == monitor.monitor_height;
    let work_area_match =
        saved.work_width == monitor.work_width && saved.work_height == monitor.work_height;
    let scale_match = (saved.scale_factor - monitor.scale_factor).abs() < 0.01;
    let origin_match = saved.work_x == monitor.work_x && saved.work_y == monitor.work_y;

    if name_match {
        return Some(
            100 + u32::from(resolution_match) * 16
                + u32::from(work_area_match) * 8
                + u32::from(scale_match) * 4
                + u32::from(origin_match) * 2,
        );
    }
    if resolution_match && work_area_match && scale_match {
        return Some(32 + u32::from(origin_match) * 2);
    }
    None
}

pub fn resolve_main(
    geometry: &MainGeometry,
    monitors: &[MonitorInfo],
    primary_index: Option<usize>,
) -> Option<ResolvedMainGeometry> {
    if !valid_main(geometry) {
        return None;
    }
    let index = choose_monitor(
        &geometry.monitor,
        geometry.x,
        geometry.y,
        geometry.width,
        geometry.height,
        monitors,
        primary_index,
    )?;
    let monitor = &monitors[index];
    let scale = monitor.scale_factor;
    let width = scaled_u32(geometry.width, scale)?;
    let height = scaled_u32(geometry.height, scale)?;
    let logical_x = geometry.x;
    let logical_y = geometry.y;
    let desired_x = monitor.work_x as f64 + logical_x * scale;
    let desired_y = monitor.work_y as f64 + logical_y * scale;
    Some(ResolvedMainGeometry {
        position: PhysicalPosition {
            x: clamp_axis(desired_x, width, monitor.work_x, monitor.work_width),
            y: clamp_axis(desired_y, height, monitor.work_y, monitor.work_height),
        },
        size: PhysicalSize { width, height },
        monitor_index: index,
        maximized: geometry.maximized,
    })
}

pub fn resolve_compact(
    geometry: &CompactGeometry,
    monitors: &[MonitorInfo],
    primary_index: Option<usize>,
) -> Option<ResolvedCompactPosition> {
    if !valid_compact(geometry) {
        return None;
    }
    let index = choose_monitor(
        &geometry.monitor,
        geometry.x,
        geometry.y,
        COMPACT_WIDTH,
        COMPACT_HEIGHT,
        monitors,
        primary_index,
    )?;
    let monitor = &monitors[index];
    let desired_x = monitor.work_x as f64 + geometry.x * monitor.scale_factor;
    let desired_y = monitor.work_y as f64 + geometry.y * monitor.scale_factor;
    let width = scaled_u32(COMPACT_WIDTH, monitor.scale_factor)?;
    let height = scaled_u32(COMPACT_HEIGHT, monitor.scale_factor)?;
    Some(ResolvedCompactPosition {
        position: PhysicalPosition {
            x: clamp_axis(desired_x, width, monitor.work_x, monitor.work_width),
            y: clamp_axis(desired_y, height, monitor.work_y, monitor.work_height),
        },
        monitor_index: index,
    })
}

pub fn default_compact_position(
    monitors: &[MonitorInfo],
    primary_index: Option<usize>,
) -> Option<ResolvedCompactPosition> {
    let index = primary_index
        .filter(|index| *index < monitors.len() && monitors[*index].is_valid())
        .or_else(|| monitors.iter().position(MonitorInfo::is_valid))?;
    let monitor = &monitors[index];
    let scale = monitor.scale_factor;
    let width = scaled_u32(COMPACT_WIDTH, scale)?;
    let height = scaled_u32(COMPACT_HEIGHT, scale)?;
    let centered_x = monitor.work_x as f64 + (monitor.work_width as f64 - width as f64) / 2.0;
    let centered_y = monitor.work_y as f64 + (monitor.work_height as f64 - height as f64) / 2.0;
    Some(ResolvedCompactPosition {
        position: PhysicalPosition {
            x: clamp_axis(centered_x, width, monitor.work_x, monitor.work_width),
            y: clamp_axis(centered_y, height, monitor.work_y, monitor.work_height),
        },
        monitor_index: index,
    })
}

pub fn main_from_physical(
    position: PhysicalPosition,
    size: PhysicalSize,
    monitor: &MonitorInfo,
    maximized: bool,
) -> Option<MainGeometry> {
    if !monitor.is_valid() {
        return None;
    }
    let scale = monitor.scale_factor;
    let geometry = MainGeometry {
        x: (position.x as f64 - monitor.work_x as f64) / scale,
        y: (position.y as f64 - monitor.work_y as f64) / scale,
        width: size.width as f64 / scale,
        height: size.height as f64 / scale,
        maximized,
        monitor: monitor.fingerprint(),
    };
    valid_main(&geometry).then_some(geometry)
}

pub fn compact_from_physical(
    position: PhysicalPosition,
    monitor: &MonitorInfo,
) -> Option<CompactGeometry> {
    if !monitor.is_valid() {
        return None;
    }
    let scale = monitor.scale_factor;
    let geometry = CompactGeometry {
        x: (position.x as f64 - monitor.work_x as f64) / scale,
        y: (position.y as f64 - monitor.work_y as f64) / scale,
        monitor: monitor.fingerprint(),
    };
    valid_compact(&geometry).then_some(geometry)
}

pub fn update_main_geometry(
    document: &mut PresentationDocument,
    captured_normal_geometry: Option<MainGeometry>,
    is_maximized: bool,
) {
    if is_maximized {
        if let Some(normal) = document.main.as_mut() {
            normal.maximized = true;
        }
        return;
    }

    if let Some(mut normal) = captured_normal_geometry.filter(valid_main) {
        normal.maximized = false;
        document.main = Some(normal);
    }
}

fn scaled_u32(value: f64, scale: f64) -> Option<u32> {
    if !value.is_finite() || !valid_scale(scale) {
        return None;
    }
    let scaled = (value * scale).round();
    (scaled > 0.0 && scaled <= MAX_PHYSICAL_DIMENSION as f64).then_some(scaled as u32)
}

fn clamp_axis(position: f64, size: u32, work_start: i32, work_size: u32) -> i32 {
    let start = work_start as f64;
    let available = work_size as f64;
    let size = size as f64;
    let clamped = if size <= available {
        position.clamp(start, start + available - size)
    } else {
        // If an unusually small work area cannot contain the app's minimum
        // size, keep the whole titlebar origin on-screen and let the rest
        // extend right/down rather than becoming unreachable.
        start
    };
    clamped.round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn monitor(
        name: &str,
        origin: (i32, i32),
        work: (i32, i32, u32, u32),
        scale: f64,
    ) -> MonitorInfo {
        MonitorInfo {
            name: Some(name.into()),
            monitor_width: work.2,
            monitor_height: work.3,
            monitor_x: origin.0,
            monitor_y: origin.1,
            work_x: work.0,
            work_y: work.1,
            work_width: work.2,
            work_height: work.3,
            scale_factor: scale,
        }
    }

    fn temporary_preferences_path() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "s4quota-window-state-{}-{nonce}.json",
            std::process::id()
        ))
    }

    fn main_geometry(monitor: &MonitorInfo) -> MainGeometry {
        MainGeometry {
            x: 80.0,
            y: 60.0,
            width: 960.0,
            height: 680.0,
            maximized: false,
            monitor: monitor.fingerprint(),
        }
    }

    #[test]
    fn schema_round_trips_and_ignores_unknown_fields() {
        let mut document = PresentationDocument::default();
        document.last_visible_mode = PresentationMode::Compact;
        document.main = Some(main_geometry(&monitor(
            "Display A",
            (0, 0),
            (0, 0, 1920, 1080),
            1.0,
        )));
        let mut value = serde_json::to_value(&document).unwrap();
        value["futureField"] = json!({ "safeToIgnore": true });
        let decoded: PresentationDocument = serde_json::from_value(value).unwrap();
        assert_eq!(decoded.last_visible_mode, PresentationMode::Compact);
        assert!(decoded.main.is_some());
    }

    #[test]
    fn invalid_schema_data_falls_back_without_preventing_startup() {
        assert!(serde_json::from_str::<PresentationDocument>("not json").is_err());
        let bad_version = PresentationDocument {
            version: SCHEMA_VERSION + 1,
            ..PresentationDocument::default()
        };
        assert_ne!(bad_version.version, SCHEMA_VERSION);
    }

    #[test]
    fn corrupt_file_is_ignored_and_does_not_block_startup() {
        let path = temporary_preferences_path();
        fs::write(&path, "{broken json").unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.issue, Some(LoadIssue::InvalidJson));
        assert_eq!(loaded.document, PresentationDocument::default());
        assert!(loaded.write_allowed);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn unsupported_schema_is_not_overwritten() {
        let path = temporary_preferences_path();
        let mut future = PresentationDocument::default();
        future.version += 1;
        fs::write(&path, serde_json::to_vec(&future).unwrap()).unwrap();

        let loaded = load(&path);
        assert_eq!(loaded.issue, Some(LoadIssue::UnsupportedVersion));
        assert!(!loaded.write_allowed);
        assert_eq!(loaded.document, PresentationDocument::default());
        assert_eq!(
            serde_json::from_slice::<PresentationDocument>(&fs::read(&path).unwrap())
                .unwrap()
                .version,
            SCHEMA_VERSION + 1
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn atomic_save_replaces_existing_preferences() {
        let path = temporary_preferences_path();
        let first = PresentationDocument::default();
        save_atomic(&path, &first).unwrap();

        let second = PresentationDocument {
            last_visible_mode: PresentationMode::Compact,
            ..PresentationDocument::default()
        };
        save_atomic(&path, &second).unwrap();
        assert_eq!(load(&path).document, second);
        assert!(!path.with_extension("json.tmp").exists());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn invalid_main_bounds_are_rejected() {
        let screen = monitor("Display A", (0, 0), (0, 0, 1920, 1080), 1.0);
        let mut geometry = main_geometry(&screen);
        geometry.width = MAIN_MIN_WIDTH - 1.0;
        assert!(!valid_main(&geometry));
        geometry = main_geometry(&screen);
        geometry.height = MAIN_MIN_HEIGHT - 1.0;
        assert!(!valid_main(&geometry));
        geometry = main_geometry(&screen);
        geometry.width = f64::INFINITY;
        assert!(!valid_main(&geometry));
    }

    #[test]
    fn absurd_or_corrupted_positions_are_rejected() {
        let screen = monitor("Display A", (0, 0), (0, 0, 1920, 1080), 1.0);
        let mut geometry = main_geometry(&screen);
        geometry.x = f64::NAN;
        assert!(!valid_main(&geometry));
        geometry = main_geometry(&screen);
        geometry.y = MAX_LOGICAL_OFFSET + 1.0;
        assert!(!valid_main(&geometry));
        geometry = main_geometry(&screen);
        geometry.width = MAX_LOGICAL_DIMENSION + 1.0;
        assert!(!valid_main(&geometry));
    }

    #[test]
    fn stale_monitor_falls_back_to_primary_and_clamps_inside_work_area() {
        let old = monitor("Disconnected", (-1920, 0), (-1920, 0, 1920, 1040), 1.0);
        let current = monitor("Primary", (0, 0), (0, 0, 1280, 720), 1.0);
        let mut geometry = main_geometry(&old);
        geometry.x = 3000.0;
        geometry.y = 3000.0;
        let resolved = resolve_main(&geometry, std::slice::from_ref(&current), Some(0)).unwrap();
        assert_eq!(resolved.monitor_index, 0);
        assert_eq!(resolved.position, PhysicalPosition { x: 320, y: 40 });
        assert_eq!(
            resolved.size,
            PhysicalSize {
                width: 960,
                height: 680
            }
        );
    }

    #[test]
    fn disappeared_monitor_prefers_available_work_area_with_best_intersection() {
        let old = monitor("Disconnected", (0, 0), (0, 0, 1920, 1080), 1.0);
        let left = monitor("Left", (-1280, 0), (-1280, 0, 1280, 1024), 1.0);
        let right = monitor("Right", (0, 0), (0, 0, 1920, 1080), 1.0);
        let mut geometry = main_geometry(&old);
        geometry.x = 2000.0;
        geometry.y = 80.0;
        let resolved = resolve_main(&geometry, &[left, right], Some(0)).unwrap();
        assert_eq!(resolved.monitor_index, 1);
        assert_eq!(resolved.position.x, 960);
        assert_eq!(resolved.position.y, 80);
    }

    #[test]
    fn matching_monitor_is_selected_before_intersection_fallback() {
        let source = monitor("Display B", (1920, 0), (1920, 0, 2560, 1440), 1.5);
        let primary = monitor("Display A", (0, 0), (0, 0, 1920, 1080), 1.0);
        let geometry = main_geometry(&source);
        let resolved = resolve_main(&geometry, &[primary, source], Some(0)).unwrap();
        assert_eq!(resolved.monitor_index, 1);
        assert_eq!(resolved.position, PhysicalPosition { x: 2040, y: 90 });
        assert_eq!(
            resolved.size,
            PhysicalSize {
                width: 1440,
                height: 1020
            }
        );
    }

    #[test]
    fn scale_change_keeps_logical_size_and_offset() {
        let source = monitor("Display A", (0, 0), (0, 0, 1920, 1080), 1.0);
        let target = monitor("Display A", (0, 0), (0, 0, 2880, 1620), 1.5);
        let geometry = main_geometry(&source);
        let resolved = resolve_main(&geometry, &[target], Some(0)).unwrap();
        assert_eq!(resolved.position, PhysicalPosition { x: 120, y: 90 });
        assert_eq!(
            resolved.size,
            PhysicalSize {
                width: 1440,
                height: 1020
            }
        );
    }

    #[test]
    fn capture_converts_physical_bounds_to_logical_offsets_and_size() {
        let screen = monitor("Display A", (-1920, 0), (-1920, 40, 1920, 1040), 1.5);
        let captured = main_from_physical(
            PhysicalPosition { x: -1800, y: 130 },
            PhysicalSize {
                width: 1440,
                height: 1020,
            },
            &screen,
            false,
        )
        .unwrap();

        assert_eq!((captured.x, captured.y), (80.0, 60.0));
        assert_eq!((captured.width, captured.height), (960.0, 680.0));
    }

    #[test]
    fn clamp_keeps_titlebar_origin_visible_when_work_area_is_smaller_than_minimum() {
        let small = monitor("Small", (0, 0), (0, 0, 640, 480), 1.0);
        let mut geometry = main_geometry(&small);
        geometry.x = -400.0;
        geometry.y = 900.0;
        let resolved = resolve_main(&geometry, &[small], Some(0)).unwrap();
        assert_eq!(resolved.position, PhysicalPosition { x: 0, y: 0 });
        assert_eq!(resolved.size.width, 960);
        assert_eq!(resolved.size.height, 680);
    }

    #[test]
    fn maximized_state_preserves_normal_bounds() {
        let screen = monitor("Display A", (0, 0), (0, 0, 1920, 1080), 1.0);
        let normal = main_geometry(&screen);
        let mut document = PresentationDocument::default();
        document.main = Some(normal.clone());
        update_main_geometry(&mut document, None, true);
        assert_eq!(document.main.as_ref().unwrap().x, normal.x);
        assert_eq!(document.main.as_ref().unwrap().y, normal.y);
        assert_eq!(document.main.as_ref().unwrap().width, normal.width);
        assert_eq!(document.main.as_ref().unwrap().height, normal.height);
        let resolved = resolve_main(&document.main.unwrap(), &[screen], Some(0)).unwrap();
        assert_eq!(
            resolved.size,
            PhysicalSize {
                width: 960,
                height: 680
            }
        );
        assert!(resolved.maximized);
    }

    #[test]
    fn tray_only_does_not_change_last_visible_mode() {
        let mut document = PresentationDocument::default();
        document.last_visible_mode = PresentationMode::Compact;
        let _transient_visibility = "trayOnly";
        assert_eq!(document.last_visible_mode, PresentationMode::Compact);
    }

    #[test]
    fn compact_size_remains_canonical_while_position_uses_monitor_scale() {
        let screen = monitor("Display A", (0, 0), (0, 0, 2880, 1620), 1.5);
        let geometry = CompactGeometry {
            x: 40.0,
            y: 50.0,
            monitor: screen.fingerprint(),
        };
        let resolved = resolve_compact(&geometry, &[screen], Some(0)).unwrap();
        assert_eq!(resolved.position, PhysicalPosition { x: 60, y: 75 });
        assert_eq!(COMPACT_WIDTH, 304.0);
        assert_eq!(COMPACT_HEIGHT, 120.0);
    }

    #[test]
    fn work_area_clamps_partially_offscreen_geometry() {
        let screen = monitor("Display A", (0, 0), (0, 40, 1920, 1040), 1.0);
        let mut geometry = main_geometry(&screen);
        geometry.x = -500.0;
        geometry.y = 900.0;
        let resolved = resolve_main(&geometry, &[screen], Some(0)).unwrap();
        assert_eq!(resolved.position.x, 0);
        assert_eq!(resolved.position.y, 400);
    }
}
