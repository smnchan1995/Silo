use gpui::{rgb, Rgba};

/// Modernist: every corner is square. Enforced as elements are added.
#[allow(dead_code)]
pub const RADIUS: f32 = 0.0;

#[derive(Clone, Debug)]
pub struct Theme {
    /// Main content background (airy paper).
    pub bg: Rgba,
    /// Rail/sidebar background (a touch warmer/darker than `bg`).
    pub surface: Rgba,
    /// Primary ink.
    pub text: Rgba,
    /// Secondary text: labels, counts, breadcrumbs.
    pub muted: Rgba,
    /// Tertiary text: placeholders, neglected/disabled rows.
    pub faint: Rgba,
    /// Hairline dividers.
    pub divider: Rgba,
    /// Signature red-orange: selection, active nav, emphasis. Used sparingly.
    pub accent: Rgba,
}

impl Theme {
    pub fn light() -> Self {
        Self {
            bg: rgb(0xf7f6f5),
            surface: rgb(0xf1efee),
            text: rgb(0x201e1d),
            muted: rgb(0x6f6a6a),
            faint: rgb(0xb3aeae),
            divider: rgb(0xe4e1e1),
            accent: rgb(0xec3013),
        }
    }

    /// Dark variant — wired to a theme toggle in a later milestone.
    #[allow(dead_code)]
    pub fn dark() -> Self {
        Self {
            bg: rgb(0x1a1918),
            surface: rgb(0x232120),
            text: rgb(0xf2efee),
            muted: rgb(0x9b9797),
            faint: rgb(0x605d5d),
            divider: rgb(0x33302f),
            accent: rgb(0xff563c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_theme_matches_modernist_tokens() {
        let t = Theme::light();
        assert_eq!(t.bg, rgb(0xf7f6f5));
        assert_eq!(t.text, rgb(0x201e1d));
        assert_eq!(t.accent, rgb(0xec3013));
    }

    #[test]
    fn dark_differs_from_light_bg() {
        assert_ne!(Theme::dark().bg, Theme::light().bg);
    }

    #[test]
    fn radius_is_zero() {
        assert_eq!(RADIUS, 0.0);
    }
}
