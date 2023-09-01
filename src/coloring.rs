use ansi_term::Style;
use once_cell::sync::OnceCell;
use std::env;

static USE_COLOR: OnceCell<bool> = OnceCell::new();

pub fn use_color() -> bool {
    *USE_COLOR.get_or_init(|| {
        if let Ok(v) = env::var("NO_COLOR") {
            let v = v.to_lowercase();
            matches!(v.as_str(), "false" | "0")
        } else {
            true
        }
    })
}

pub trait MaybeColor
where
    Self: Sized,
{
    fn default_color() -> Self;

    fn maybe_color(self) -> Self {
        if use_color() {
            self
        } else {
            Self::default_color()
        }
    }
}

impl MaybeColor for Style {
    fn default_color() -> Self {
        Self::new()
    }
}
