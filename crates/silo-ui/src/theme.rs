use gpui::{rgb, Rgba};

/// Modernist: every corner is square. Enforced as elements are added.
#[allow(dead_code)]
pub const RADIUS: f32 = 0.0;

#[derive(Clone, Debug)]
pub struct Theme {
    pub bg: Rgba,
    pub surface: Rgba,
    pub text: Rgba,
    pub divider: Rgba,
    /// Used for selection highlight / active nav (wired in later milestones).
    #[allow(dead_code)]
    pub accent: Rgba,
}

impl Theme {
    pub fn light() -> Self {
        Self {
            bg: rgb(0xf3f2f2),
            surface: rgb(0xeae9e9),
            text: rgb(0x201e1d),
            divider: rgb(0x605d5d),
            accent: rgb(0xec3013),
        }
    }

    /// Dark variant — wired to a theme toggle in the M2 (Edit & Save) milestone.
    #[allow(dead_code)]
    pub fn dark() -> Self {
        Self {
            bg: rgb(0x201e1d),
            surface: rgb(0x2d2b2b),
            text: rgb(0xf8f4f4),
            divider: rgb(0x605d5d),
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
        assert_eq!(t.bg, rgb(0xf3f2f2));
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
