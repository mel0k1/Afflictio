pub mod font;
pub mod ioc;
pub mod kms;

use font::{draw_text, fill_rect, GLYPH_HEIGHT, GLYPH_WIDTH};
use serde_json::json;

pub const BACKGROUND: u32 = 0xFF1A1E2A;
pub const TITLE_COLOR: u32 = 0xFF8BE9FD;
pub const TEXT_COLOR: u32 = 0xFFF8F8F2;
pub const BORDER_COLOR: u32 = 0xFF6272A4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub color: u32,
    pub label: String,
}

pub fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (0xFF << 24) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

pub fn compose(
    buffer: &mut [u32],
    pitch_px: u32,
    width: u32,
    height: u32,
    frame: u32,
    windows: &[Window],
    title: &str,
) {
    fill_rect(buffer, pitch_px, 0, 0, width, height, BACKGROUND);
    if width > 8 * GLYPH_WIDTH && height > GLYPH_HEIGHT * 3 {
        draw_text(buffer, pitch_px, title, 8, 6, 2, TITLE_COLOR);
    }
    for window in windows {
        draw_window(buffer, pitch_px, width, height, window, frame);
    }
    if height > GLYPH_HEIGHT * 3 {
        let counter = format!("FRAME {frame}");
        let text_width = counter.chars().count() as u32 * (GLYPH_WIDTH + 1);
        let x = width.saturating_sub(text_width * 2 + 12);
        draw_text(
            buffer,
            pitch_px,
            &counter,
            x,
            height - GLYPH_HEIGHT * 2 - 4,
            2,
            TEXT_COLOR,
        );
    }
}

fn draw_window(
    buffer: &mut [u32],
    pitch_px: u32,
    width: u32,
    height: u32,
    window: &Window,
    frame: u32,
) {
    if window.x >= width || window.y >= height {
        return;
    }
    fill_rect(
        buffer,
        pitch_px,
        window.x,
        window.y,
        window.w,
        window.h,
        BORDER_COLOR,
    );
    if window.w > 4 && window.h > 4 {
        let inner_w = window.w - 4;
        let inner_h = window.h - 4;
        fill_rect(
            buffer,
            pitch_px,
            window.x + 2,
            window.y + 2,
            inner_w,
            inner_h,
            window.color,
        );
        let bar_h = 6.max(inner_h / 8);
        let travel = inner_h.saturating_sub(bar_h);
        let bar_y = window.y + 2 + (frame % (travel + 1));
        fill_rect(
            buffer,
            pitch_px,
            window.x + 2,
            bar_y,
            inner_w,
            bar_h,
            rgb(250, 241, 137),
        );
    }
    if window.w > 40 && window.h > GLYPH_HEIGHT + 8 {
        draw_text(
            buffer,
            pitch_px,
            &window.label,
            window.x + 6,
            window.y + 4,
            1,
            TEXT_COLOR,
        );
    }
}

pub fn status_json(connector: &str, mode: &str, method: &str, flips: u32, frames: u32) -> String {
    json!({
        "connector": connector,
        "mode": mode,
        "method": method,
        "flips": flips,
        "frames": frames,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_paints_background_and_windows() {
        let width = 320u32;
        let height = 160u32;
        let mut buffer = vec![0u32; (width * height) as usize];
        let windows = vec![Window {
            x: 16,
            y: 24,
            w: 120,
            h: 80,
            color: rgb(80, 60, 110),
            label: String::from("VT1"),
        }];
        compose(&mut buffer, width, width, height, 7, &windows, "AFFLICTIO");
        assert_eq!(buffer[0], BACKGROUND);
        let border_index = (24 * width + 16) as usize;
        assert_eq!(buffer[border_index], BORDER_COLOR);
        let inner_index = ((24 + 8) * width + 16 + 8) as usize;
        assert_eq!(buffer[inner_index], rgb(80, 60, 110));
    }

    #[test]
    fn moving_bar_changes_with_frame() {
        let width = 160u32;
        let height = 120u32;
        let windows = vec![Window {
            x: 8,
            y: 8,
            w: 100,
            h: 100,
            color: rgb(10, 10, 10),
            label: String::from("W"),
        }];
        let mut a = vec![0u32; (width * height) as usize];
        compose(&mut a, width, width, height, 0, &windows, "T");
        let mut b = vec![0u32; (width * height) as usize];
        compose(&mut b, width, width, height, 3, &windows, "T");
        let diff = a.iter().zip(b.iter()).filter(|(x, y)| x != y).count();
        assert!(diff > 0, "bar animation must move pixels");
    }

    #[test]
    fn windows_offscreen_are_ignored() {
        let width = 64u32;
        let height = 48u32;
        let windows = vec![Window {
            x: 200,
            y: 200,
            w: 50,
            h: 50,
            color: 0xFF000000,
            label: String::new(),
        }];
        let mut with_window = vec![0u32; (width * height) as usize];
        compose(&mut with_window, width, width, height, 0, &windows, "");
        let mut without = vec![0u32; (width * height) as usize];
        compose(&mut without, width, width, height, 0, &[], "");
        assert_eq!(with_window, without);
    }

    #[test]
    fn status_json_fields() {
        let status = status_json("Virtual-1", "1024x768", "pageflip", 42, 42);
        assert!(status.contains("\"connector\":\"Virtual-1\""));
        assert!(status.contains("\"method\":\"pageflip\""));
        assert!(status.contains("\"flips\":42"));
    }

    #[test]
    fn compose_frame_counter_drawn() {
        let width = 320u32;
        let height = 160u32;
        let mut buffer = vec![0u32; (width * height) as usize];
        compose(&mut buffer, width, width, height, 0, &[], "T");
        let painted = buffer.iter().filter(|p| **p != BACKGROUND).count();
        assert!(painted > 100, "title and counter painted {painted} px");
    }
}
