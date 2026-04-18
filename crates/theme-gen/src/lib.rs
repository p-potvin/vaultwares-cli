use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultTheme {
    pub name: &'static str,
    pub mode: &'static str,
    pub primary: &'static str,
    pub accent: &'static str,
    pub slug: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThemePalette {
    pub theme_name: &'static str,
    pub theme_slug: &'static str,
    pub mode: &'static str,
    pub background: &'static str,
    pub surface: &'static str,
    pub surface_elevated: &'static str,
    pub text_primary: &'static str,
    pub text_secondary: &'static str,
    pub accent: &'static str,
    pub accent_hover: &'static str,
    pub border_subtle: &'static str,
    pub focus_ring: &'static str,
}

pub const DEFAULT_THEMES: &[VaultTheme] = &[
    VaultTheme {
        name: "Cyberpunk Cinder",
        mode: "dark",
        primary: "#073642",
        accent: "#CB4B16",
        slug: "cyberpunk-cinder",
    },
    VaultTheme {
        name: "Golden Slate",
        mode: "dark",
        primary: "#4A5459",
        accent: "#D4AF37",
        slug: "golden-slate",
    },
    VaultTheme {
        name: "Ocean Mist",
        mode: "light",
        primary: "#D3D3D3",
        accent: "#006994",
        slug: "ocean-mist",
    },
];

pub fn default_palettes() -> Vec<ThemePalette> {
    vec![
        ThemePalette {
            theme_name: "Cyberpunk Cinder",
            theme_slug: "cyberpunk-cinder",
            mode: "dark",
            background: "#073642",
            surface: "#17414D",
            surface_elevated: "#224D58",
            text_primary: "#F8FAFC",
            text_secondary: "#CBD5E1",
            accent: "#CB4B16",
            accent_hover: "#D66538",
            border_subtle: "#395C66",
            focus_ring: "#D77A4B",
        },
        ThemePalette {
            theme_name: "Golden Slate",
            theme_slug: "golden-slate",
            mode: "dark",
            background: "#4A5459",
            surface: "#576166",
            surface_elevated: "#616C71",
            text_primary: "#F8FAFC",
            text_secondary: "#CBD5E1",
            accent: "#D4AF37",
            accent_hover: "#DBBB59",
            border_subtle: "#6A7377",
            focus_ring: "#DEC56A",
        },
        ThemePalette {
            theme_name: "Ocean Mist",
            theme_slug: "ocean-mist",
            mode: "light",
            background: "#D3D3D3",
            surface: "#CBD6D8",
            surface_elevated: "#C4D8DD",
            text_primary: "#111827",
            text_secondary: "#4B5563",
            accent: "#006994",
            accent_hover: "#005B81",
            border_subtle: "#A9B1B6",
            focus_ring: "#2A77A0",
        },
    ]
}

pub fn palette_by_slug(slug: &str) -> Result<ThemePalette> {
    default_palettes()
        .into_iter()
        .find(|palette| palette.theme_slug == slug)
        .with_context(|| format!("unknown theme slug `{slug}`"))
}

#[cfg(test)]
mod tests {
    use super::{default_palettes, palette_by_slug};

    #[test]
    fn exposes_default_vaultwares_palettes() {
        let palettes = default_palettes();
        assert_eq!(palettes.len(), 3);
        assert_eq!(
            palette_by_slug("golden-slate")
                .expect("theme should exist")
                .accent,
            "#D4AF37"
        );
    }
}
