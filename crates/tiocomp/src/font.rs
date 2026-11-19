pub struct Glyph {
    pub rows: [u8; 7],
}

pub const GLYPH_WIDTH: u32 = 5;
pub const GLYPH_HEIGHT: u32 = 7;

const A: Glyph = Glyph {
    rows: [
        0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
    ],
};
const B: Glyph = Glyph {
    rows: [
        0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
    ],
};
const C: Glyph = Glyph {
    rows: [
        0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
    ],
};
const D: Glyph = Glyph {
    rows: [
        0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
    ],
};
const E: Glyph = Glyph {
    rows: [
        0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
    ],
};
const F: Glyph = Glyph {
    rows: [
        0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
    ],
};
const I: Glyph = Glyph {
    rows: [
        0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
    ],
};
const K: Glyph = Glyph {
    rows: [
        0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
    ],
};
const L: Glyph = Glyph {
    rows: [
        0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
    ],
};
const M: Glyph = Glyph {
    rows: [
        0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
    ],
};
const O: Glyph = Glyph {
    rows: [
        0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
    ],
};
const P: Glyph = Glyph {
    rows: [
        0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
    ],
};
const R: Glyph = Glyph {
    rows: [
        0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
    ],
};
const S: Glyph = Glyph {
    rows: [
        0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
    ],
};
const T: Glyph = Glyph {
    rows: [
        0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
    ],
};
const U: Glyph = Glyph {
    rows: [
        0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
    ],
};
const N0: Glyph = Glyph {
    rows: [
        0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
    ],
};
const N1: Glyph = Glyph {
    rows: [
        0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
    ],
};
const N2: Glyph = Glyph {
    rows: [
        0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
    ],
};
const N3: Glyph = Glyph {
    rows: [
        0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110,
    ],
};
const N4: Glyph = Glyph {
    rows: [
        0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
    ],
};
const N5: Glyph = Glyph {
    rows: [
        0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
    ],
};
const N6: Glyph = Glyph {
    rows: [
        0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
    ],
};
const N7: Glyph = Glyph {
    rows: [
        0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
    ],
};
const N8: Glyph = Glyph {
    rows: [
        0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
    ],
};
const N9: Glyph = Glyph {
    rows: [
        0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
    ],
};
const SPACE: Glyph = Glyph {
    rows: [0, 0, 0, 0, 0, 0, 0],
};
const COLON: Glyph = Glyph {
    rows: [0, 0b00100, 0b00100, 0, 0b00100, 0b00100, 0],
};
const DOT: Glyph = Glyph {
    rows: [0, 0, 0, 0, 0, 0b01100, 0b01100],
};
const DASH: Glyph = Glyph {
    rows: [0, 0, 0, 0b11111, 0, 0, 0],
};

pub fn glyph(ch: char) -> Option<&'static Glyph> {
    match ch {
        'A' => Some(&A),
        'B' => Some(&B),
        'C' => Some(&C),
        'D' => Some(&D),
        'E' => Some(&E),
        'F' => Some(&F),
        'I' => Some(&I),
        'K' => Some(&K),
        'L' => Some(&L),
        'M' => Some(&M),
        'O' => Some(&O),
        'P' => Some(&P),
        'R' => Some(&R),
        'S' => Some(&S),
        'T' => Some(&T),
        'U' => Some(&U),
        '0' => Some(&N0),
        '1' => Some(&N1),
        '2' => Some(&N2),
        '3' => Some(&N3),
        '4' => Some(&N4),
        '5' => Some(&N5),
        '6' => Some(&N6),
        '7' => Some(&N7),
        '8' => Some(&N8),
        '9' => Some(&N9),
        ' ' => Some(&SPACE),
        ':' => Some(&COLON),
        '.' => Some(&DOT),
        '-' => Some(&DASH),
        _ => None,
    }
}

pub fn draw_text(
    buffer: &mut [u32],
    pitch_pixels: u32,
    text: &str,
    x: u32,
    y: u32,
    scale: u32,
    color: u32,
) {
    let mut cursor = x;
    for ch in text.chars() {
        if let Some(g) = glyph(ch) {
            for (row, bits) in g.rows.iter().enumerate() {
                for col in 0..GLYPH_WIDTH {
                    if bits & (1 << (4 - col)) != 0 {
                        fill_rect(
                            buffer,
                            pitch_pixels,
                            cursor + col * scale,
                            y + row as u32 * scale,
                            scale,
                            scale,
                            color,
                        );
                    }
                }
            }
        }
        cursor += (GLYPH_WIDTH + 1) * scale;
    }
}

pub fn fill_rect(
    buffer: &mut [u32],
    pitch_pixels: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    color: u32,
) {
    let pitch = pitch_pixels as usize;
    for row in y..y + h {
        for col in x..x + w {
            let index = row as usize * pitch + col as usize;
            if index < buffer.len() {
                buffer[index] = color;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paint_mask(buffer: &[u32]) -> Vec<bool> {
        buffer.iter().map(|p| *p != 0).collect()
    }

    #[test]
    fn glyph_table_covers_status_text() {
        for ch in "AFFLICTIO KMS OK 0123456789:".chars() {
            assert!(glyph(ch).is_some(), "missing glyph {ch}");
        }
    }

    #[test]
    fn glyph_rows_are_seven() {
        for ch in 'A'..='Z' {
            if let Some(g) = glyph(ch) {
                assert_eq!(g.rows.len(), 7);
            }
        }
    }

    #[test]
    fn text_draws_pixels_with_scale() {
        let width = 40u32;
        let height = 10u32;
        let mut buffer = vec![0u32; (width * height) as usize];
        draw_text(&mut buffer, width, "A", 0, 0, 1, 0xFFFFFFFF);
        let painted = paint_mask(&buffer).iter().filter(|p| **p).count();
        assert!(painted > 10, "glyph painted {painted} pixels");
    }

    #[test]
    fn fill_rect_bounds_are_safe() {
        let width = 4u32;
        let mut buffer = vec![0u32; (width * 4) as usize];
        fill_rect(&mut buffer, width, 2, 2, 8, 8, 0x00FF0000);
        assert_eq!(buffer[0], 0);
        assert_eq!(buffer[(2 * width + 2) as usize], 0x00FF0000);
        assert_eq!(buffer.last(), Some(&0x00FF0000));
    }

    #[test]
    fn colon_has_two_dots() {
        let g = glyph(':').unwrap();
        let lit: u32 = g.rows.iter().filter(|r| **r != 0).count() as u32;
        assert_eq!(lit, 4);
    }
}
