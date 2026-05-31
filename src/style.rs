#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

impl Color {
    pub const fn rgba(red: f64, green: f64, blue: f64, alpha: f64) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
}

pub const OVERLAY: Color = Color::rgba(0.025, 0.025, 0.025, 0.58);
pub const ACCENT: Color = Color::rgba(0.12549019607843137, 0.32941176470588235, 1.0, 1.0);
pub const ACCENT_HOVER: Color = Color::rgba(0.17647058823529413, 0.3843137254901961, 1.0, 1.0);
pub const TOOLBAR: Color = Color::rgba(0.09, 0.09, 0.10, 1.0);
pub const TOOLBAR_BORDER: Color = Color::rgba(0.24, 0.24, 0.26, 1.0);
pub const SECONDARY_BUTTON: Color = Color::rgba(0.16, 0.16, 0.18, 1.0);
pub const SECONDARY_BUTTON_HOVER: Color = Color::rgba(0.22, 0.22, 0.24, 1.0);
pub const SHADOW: Color = Color::rgba(0.0, 0.0, 0.0, 0.34);
pub const TEXT: Color = Color::rgba(0.96, 0.96, 0.96, 1.0);
pub const MUTED_TEXT: Color = Color::rgba(0.72, 0.72, 0.72, 1.0);
pub const HANDLE_FILL: Color = Color::rgba(0.98, 0.98, 0.98, 1.0);

pub const HANDLE_SIZE: f64 = 10.0;
pub const HANDLE_HIT_RADIUS: f64 = 12.0;
pub const TOOLBAR_WIDTH: f64 = 146.0;
pub const TOOLBAR_HEIGHT: f64 = 52.0;
pub const TOOLBAR_GAP: f64 = 14.0;
pub const TOOLBAR_PADDING: f64 = 8.0;
pub const TOOLBAR_CONTENT_GAP: f64 = 6.0;
pub const CANCEL_BUTTON_WIDTH: f64 = 36.0;
pub const SELECT_BUTTON_WIDTH: f64 = 88.0;
pub const SELECT_BUTTON_HEIGHT: f64 = 36.0;
