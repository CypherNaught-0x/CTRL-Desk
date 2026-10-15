//! Persistent user settings, stored as JSON in the platform config directory.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// The desk chosen during onboarding. `None` until onboarding completes.
    pub desk: Option<SavedDesk>,
    /// Overrides the controller-reported tabletop height at the lowest position.
    pub base_height_mm: Option<f32>,
    /// Highest position of the desk. Only used for display and the illustration.
    pub max_height_mm: Option<f32>,
    pub theme: ThemeChoice,
    pub primary_light: [u8; 3],
    pub primary_dark: [u8; 3],
    pub favourites: Vec<Favourite>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedDesk {
    pub name: String,
    /// [`linak_desk::device_id`]: MAC address, or a UUID on macOS.
    #[serde(alias = "address")]
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Favourite {
    pub name: String,
    pub height_mm: f32,
    pub icon: FavIcon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FavIcon {
    Sitting,
    Standing,
    Laptop,
    Coffee,
    Book,
    Star,
}

impl FavIcon {
    pub const ALL: [FavIcon; 6] = [
        FavIcon::Sitting,
        FavIcon::Standing,
        FavIcon::Laptop,
        FavIcon::Coffee,
        FavIcon::Book,
        FavIcon::Star,
    ];

    pub fn glyph(self) -> &'static str {
        use egui_phosphor::regular as i;
        match self {
            FavIcon::Sitting => i::OFFICE_CHAIR,
            FavIcon::Standing => i::PERSON,
            FavIcon::Laptop => i::LAPTOP,
            FavIcon::Coffee => i::COFFEE,
            FavIcon::Book => i::BOOK_OPEN,
            FavIcon::Star => i::STAR,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeChoice {
    System,
    Light,
    Dark,
}

pub const DEFAULT_PRIMARY_LIGHT: [u8; 3] = [0x2f, 0x5a, 0x40];
pub const DEFAULT_PRIMARY_DARK: [u8; 3] = [0xd9, 0xd9, 0xd9];

/// Typical travel of a Linak DPG desk frame (raw 0..6500).
pub const DEFAULT_TRAVEL_MM: f32 = 650.0;

impl Default for Config {
    fn default() -> Self {
        Self {
            desk: None,
            base_height_mm: None,
            max_height_mm: None,
            theme: ThemeChoice::System,
            primary_light: DEFAULT_PRIMARY_LIGHT,
            primary_dark: DEFAULT_PRIMARY_DARK,
            favourites: vec![
                Favourite {
                    name: "Sitting".into(),
                    height_mm: 720.0,
                    icon: FavIcon::Sitting,
                },
                Favourite {
                    name: "Standing".into(),
                    height_mm: 1100.0,
                    icon: FavIcon::Standing,
                },
                Favourite {
                    name: "Focus".into(),
                    height_mm: 980.0,
                    icon: FavIcon::Laptop,
                },
            ],
        }
    }
}

/// Demo mode (`--demo` or `DESK_CTRL_DEMO=1`) simulates a desk instead of using Bluetooth.
pub fn is_demo() -> bool {
    static DEMO: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *DEMO.get_or_init(|| {
        std::env::args().any(|a| a == "--demo")
            || std::env::var_os("DESK_CTRL_DEMO").is_some_and(|v| !v.is_empty() && v != "0")
    })
}

impl Config {
    fn path() -> Option<PathBuf> {
        if let Some(p) = std::env::var_os("DESK_CTRL_CONFIG") {
            return Some(p.into());
        }
        // Keep the simulated desk out of the real configuration.
        let file = if is_demo() {
            "config-demo.json"
        } else {
            "config.json"
        };
        Some(dirs::config_dir()?.join("desk-ctrl").join(file))
    }

    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}
