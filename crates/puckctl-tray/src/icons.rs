use std::sync::LazyLock;

use image::GenericImageView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Color,
    Steam,
    Dim,
}

/// Steam client accent (`#66C0F4`).
const STEAM_R: u16 = 0x66;
const STEAM_G: u16 = 0xC0;
const STEAM_B: u16 = 0xF4;

fn load_png(bytes: &[u8], style: Style) -> ksni::Icon {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .expect("embedded tray png");
    let (width, height) = img.dimensions();
    let mut data = img.into_rgba8().into_vec();
    for pixel in data.as_chunks_mut::<4>().0 {
        match style {
            Style::Color => {}
            Style::Steam => {
                let luma = (u16::from(pixel[0]) + u16::from(pixel[1]) + u16::from(pixel[2])) / 3;
                pixel[0] = u8::try_from((luma * STEAM_R) / 255).unwrap_or(0);
                pixel[1] = u8::try_from((luma * STEAM_G) / 255).unwrap_or(0);
                pixel[2] = u8::try_from((luma * STEAM_B) / 255).unwrap_or(0);
            }
            Style::Dim => {
                let luma = (u16::from(pixel[0]) + u16::from(pixel[1]) + u16::from(pixel[2])) / 3;
                let grey = u8::try_from((luma * 5) / 10).unwrap_or(0);
                pixel[0] = grey;
                pixel[1] = grey;
                pixel[2] = grey;
                pixel[3] = u8::try_from((u16::from(pixel[3]) * 6) / 10).unwrap_or(0);
            }
        }
        pixel.rotate_right(1);
    }
    ksni::Icon {
        width: i32::try_from(width).unwrap_or(64),
        height: i32::try_from(height).unwrap_or(64),
        data,
    }
}

macro_rules! png {
    ($bytes:expr, $style:expr) => {
        LazyLock::new(|| load_png($bytes, $style))
    };
}

static GAMEPAD: LazyLock<ksni::Icon> = png!(include_bytes!("../assets/gamepad.png"), Style::Color);
static GAMEPAD_STEAM: LazyLock<ksni::Icon> =
    png!(include_bytes!("../assets/gamepad.png"), Style::Steam);
static GAMEPAD_DIM: LazyLock<ksni::Icon> =
    png!(include_bytes!("../assets/gamepad.png"), Style::Dim);
static DESKTOP: LazyLock<ksni::Icon> = png!(include_bytes!("../assets/desktop.png"), Style::Color);
static DESKTOP_STEAM: LazyLock<ksni::Icon> =
    png!(include_bytes!("../assets/desktop.png"), Style::Steam);
static DESKTOP_DIM: LazyLock<ksni::Icon> =
    png!(include_bytes!("../assets/desktop.png"), Style::Dim);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Gamepad,
    Desktop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Color,
    Steam,
    Dim,
}

#[must_use]
pub fn icon(kind: Kind, tone: Tone) -> ksni::Icon {
    match (kind, tone) {
        (Kind::Gamepad, Tone::Color) => GAMEPAD.clone(),
        (Kind::Gamepad, Tone::Steam) => GAMEPAD_STEAM.clone(),
        (Kind::Gamepad, Tone::Dim) => GAMEPAD_DIM.clone(),
        (Kind::Desktop, Tone::Color) => DESKTOP.clone(),
        (Kind::Desktop, Tone::Steam) => DESKTOP_STEAM.clone(),
        (Kind::Desktop, Tone::Dim) => DESKTOP_DIM.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_kinds_have_pixels() {
        for kind in [Kind::Gamepad, Kind::Desktop] {
            for tone in [Tone::Color, Tone::Steam, Tone::Dim] {
                let icon = icon(kind, tone);
                assert!(icon.width > 0);
                assert!(icon.height > 0);
                assert!(!icon.data.is_empty());
            }
        }
    }
}
