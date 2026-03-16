use fontdue::{Font, FontSettings};

const BG: u32 = 0xFF1E1E2E;
const FG: u32 = 0xE6E6E6FF;
const FG_DIM: u32 = 0x73C0CAF5;
const FG_HINT: u32 = 0x4DC0CAF5;
const SEL_BG: u32 = 0x15C0CAF5;
const LINE: u32 = 0xFF2A2A3E;

const FONT_SIZE: f32 = 22.0;
const HINT_SIZE: f32 = 25.0;
const ROW_H: u32 = 35;
const INPUT_H: u32 = 45;
const PAD_X: u32 = 16;

pub struct Renderer {
    font: Font,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub max_visible_rows: u32,
}

impl Renderer {
    pub fn new(width: u32, height: u32, scale: f32) -> Self {
        let font_paths = [
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/TTF/NotoSans-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/noto-sans/NotoSans-Regular.ttf",
            "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
        ];
        let font_data = font_paths
            .iter()
            .find_map(|p| std::fs::read(p).ok())
            .unwrap_or_else(|| panic!("No font found. Install noto-fonts."));
        let font = Font::from_bytes(font_data, FontSettings::default()).unwrap();
        let max_visible_rows: u32 = {
            let row_h = (ROW_H as f32 * scale).round() as u32;
            let input_h = (INPUT_H as f32 * scale).round() as u32;
            (height.saturating_sub(input_h + 1) / row_h) as u32
        };

        Self {
            font,
            width,
            height,
            scale,
            max_visible_rows: max_visible_rows,
        }
    }

    pub fn render(
        &self,
        query: &str,
        results: &[crate::search::LauncherItem],
        selected: usize,
        visible: usize,
    ) -> Vec<u32> {
        let mut buf = vec![BG; (self.width * self.height) as usize];

        let font_size = (FONT_SIZE * self.scale).round();
        let hint_size = (HINT_SIZE * self.scale).round();

        let row_h = (ROW_H as f32 * self.scale).round() as u32;
        let input_h = (INPUT_H as f32 * self.scale).round() as u32;
        let pad_x = (PAD_X as f32 * self.scale).round() as u32;
        let text_x = pad_x + (20.0 * self.scale).round() as u32;

        let text_y = input_h.saturating_sub(font_size as u32) / 2;
        let cursor_symbol = "|";
        let mut cursor_x = text_x;

        if query.is_empty() {
            self.draw_text(&mut buf, "", text_x, text_y, FG_HINT, hint_size);
        } else {
            let text_end = self.draw_text(&mut buf, query, text_x, text_y, FG, font_size);
            cursor_x = text_end - 4;
        }

        self.draw_text(&mut buf, cursor_symbol, cursor_x, text_y, FG, font_size);

        // Separator
        self.draw_rect(&mut buf, 0, input_h, self.width, 1, LINE);

        // Result rows
        let max_visible_rows = (self.height.saturating_sub(input_h + 1) / row_h) as usize;
        for (i, item) in results
            .iter()
            .skip(visible)
            .take(max_visible_rows)
            .enumerate()
        {
            let row_y = input_h + 1 + i as u32 * row_h;

            let real_index = visible + i;
            if real_index == selected {
                self.draw_rect(&mut buf, 0, row_y, self.width, row_h, SEL_BG);
            }

            if row_y + row_h > self.height {
                break;
            }
            let text_y = row_y + row_h.saturating_sub(font_size as u32) / 2 - 3;
            let name_end = self.draw_text(&mut buf, &item.name, pad_x, text_y, FG, font_size);
            let cmd = format!(" {}", item.entry.command);
            self.draw_text(&mut buf, &cmd, name_end + 8, text_y, FG_DIM, font_size);
        }

        buf
    }

    fn draw_text(
        &self,
        buf: &mut Vec<u32>,
        text: &str,
        x: u32,
        y: u32,
        color: u32,
        size: f32,
    ) -> u32 {
        let size = size.round();

        let mut cx = x as i32;
        let [_, fg_r, fg_g, fg_b] = color.to_be_bytes();

        for ch in text.chars() {
            let (metrics, bitmap) = self.font.rasterize(ch, size);

            let glyph_x = cx + metrics.xmin;
            let glyph_y = y as i32 + (size as i32 - metrics.height as i32 - metrics.ymin);

            for gy in 0..metrics.height {
                for gx in 0..metrics.width {
                    let coverage = bitmap[gy * metrics.width + gx];
                    if coverage == 0 {
                        continue;
                    }

                    let px = glyph_x + gx as i32;
                    let py = glyph_y + gy as i32;

                    if px < 0 || py < 0 {
                        continue;
                    }
                    let px = px as u32;
                    let py = py as u32;
                    if px >= self.width || py >= self.height {
                        continue;
                    }

                    let idx = (py * self.width + px) as usize;
                    let a = coverage as u32;
                    let ia = 255 - a;
                    let bg = buf[idx];
                    let [_, bg_r, bg_g, bg_b] = bg.to_be_bytes();
                    let r = (fg_r as u32 * a + bg_r as u32 * ia) / 255;
                    let g = (fg_g as u32 * a + bg_g as u32 * ia) / 255;
                    let b = (fg_b as u32 * a + bg_b as u32 * ia) / 255;
                    buf[idx] = 0xFF000000 | (r << 16) | (g << 8) | b;
                }
            }

            cx += metrics.advance_width as i32;
        }

        cx.max(x as i32) as u32
    }

    fn draw_rect(&self, buf: &mut Vec<u32>, x: u32, y: u32, w: u32, h: u32, color: u32) {
        let a = (color >> 24) as u32;
        if a == 0xFF {
            for row in y..(y + h).min(self.height) {
                let start = (row * self.width + x) as usize;
                let end = (row * self.width + (x + w).min(self.width)) as usize;
                buf[start..end].fill(color);
            }
        } else {
            let [_, cr, cg, cb] = color.to_be_bytes();
            let ia = 255 - a;
            for row in y..(y + h).min(self.height) {
                for col in x..(x + w).min(self.width) {
                    let idx = (row * self.width + col) as usize;
                    let bg = buf[idx];
                    let [_, br, bg_g, bb] = bg.to_be_bytes();
                    let r = (cr as u32 * a + br as u32 * ia) / 255;
                    let g = (cg as u32 * a + bg_g as u32 * ia) / 255;
                    let b = (cb as u32 * a + bb as u32 * ia) / 255;
                    buf[idx] = 0xFF000000 | (r << 16) | (g << 8) | b;
                }
            }
        }
    }
}
