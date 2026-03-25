use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;

use memmap2::Mmap;
use swash::scale::image::Image;
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::zeno::Format;
use swash::{CacheKey, FontRef, GlyphId};

const BG: u32 = 0xFF1E1E2E;
const FG: u32 = 0xE6E6E6FF;
const FG_DIM: u32 = 0x73C0CAF5;
const FG_HINT: u32 = 0x4DC0CAF5;
const SEL_BG: u32 = 0x15C0CAF5;
const LINE: u32 = 0xFF2A2A3E;

const FONT_SIZE: f32 = 22.0;
const HINT_SIZE: f32 = 22.0;
const ROW_H: u32 = 58;
const INPUT_H: u32 = 45;
const PAD_X: u32 = 16;
const INPUT_LETTER_SPACING: f32 = 1.0;

const PRIMARY_FONT_PATHS: &[&str] = &[
    "/usr/share/fonts/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/TTF/NotoSans-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/noto-sans/NotoSans-Regular.ttf",
    "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
];

const CJK_FONT_PATHS: &[&str] = &[
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansSC-Regular.otf",
    "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/OTF/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/adobe-source-han-sans/SourceHanSans-Regular.ttc",
];

struct MappedFont {
    _mmap: Mmap,
    offset: u32,
    key: CacheKey,
}

impl MappedFont {
    fn open(path: &str) -> Option<Self> {
        let file = File::open(path).ok()?;
        let mmap = unsafe { Mmap::map(&file).ok()? };
        let font_ref = FontRef::from_index(unsafe { Self::mmap_as_static(&mmap) }, 0)?;
        Some(Self {
            _mmap: mmap,
            offset: font_ref.offset,
            key: font_ref.key,
        })
    }

    fn as_ref(&self) -> FontRef<'_> {
        FontRef {
            data: self.data(),
            offset: self.offset,
            key: self.key,
        }
    }

    fn data(&self) -> &[u8] {
        &self._mmap
    }

    unsafe fn mmap_as_static(mmap: &Mmap) -> &'static [u8] {
        unsafe { std::mem::transmute::<&[u8], &'static [u8]>(mmap.as_ref()) }
    }
}

struct CachedGlyph {
    placement_left: i32,
    placement_top: i32,
    width: u32,
    height: u32,
    advance: f32,
    data: Vec<u8>,
}

pub struct Renderer {
    primary: MappedFont,
    cjk: Option<MappedFont>,
    context: RefCell<ScaleContext>,
    cache: RefCell<HashMap<(char, u32), CachedGlyph>>,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub max_visible_rows: u32,
}

impl Renderer {
    pub fn new(width: u32, height: u32, scale: f32) -> Self {
        let primary = PRIMARY_FONT_PATHS
            .iter()
            .find_map(|p| MappedFont::open(p))
            .unwrap_or_else(|| panic!("No font found. Install noto-fonts."));

        let cjk = CJK_FONT_PATHS.iter().find_map(|p| MappedFont::open(p));

        let max_visible_rows = {
            let row_h = (ROW_H as f32 * scale).round() as u32;
            let input_h = (INPUT_H as f32 * scale).round() as u32;
            height.saturating_sub(input_h + 1) / row_h
        };

        Self {
            primary,
            cjk,
            context: RefCell::new(ScaleContext::new()),
            cache: RefCell::new(HashMap::new()),
            width,
            height,
            scale,
            max_visible_rows,
        }
    }

    fn resolve_glyph(&self, ch: char) -> (FontRef<'_>, GlyphId) {
        let primary = self.primary.as_ref();
        let gid = primary.charmap().map(ch);
        if gid != 0 {
            return (primary, gid);
        }
        if let Some(cjk) = &self.cjk {
            let cjk_ref = cjk.as_ref();
            let gid = cjk_ref.charmap().map(ch);
            if gid != 0 {
                return (cjk_ref, gid);
            }
        }
        (primary, 0)
    }

    fn rasterize_glyph(&self, ch: char, size: f32) -> std::cell::Ref<'_, CachedGlyph> {
        let key = (ch, size.to_bits());
        if !self.cache.borrow().contains_key(&key) {
            let (font_ref, gid) = self.resolve_glyph(ch);
            let advance = font_ref.glyph_metrics(&[]).scale(size).advance_width(gid);
            let mut ctx = self.context.borrow_mut();
            let mut scaler = ctx.builder(font_ref).size(size).hint(true).build();
            let image: Option<Image> = Render::new(&[
                Source::ColorOutline(0),
                Source::ColorBitmap(StrikeWith::BestFit),
                Source::Outline,
            ])
            .format(Format::Alpha)
            .render(&mut scaler, gid);

            let cached = if let Some(img) = image {
                CachedGlyph {
                    placement_left: img.placement.left,
                    placement_top: img.placement.top,
                    width: img.placement.width,
                    height: img.placement.height,
                    advance,
                    data: img.data,
                }
            } else {
                CachedGlyph {
                    placement_left: 0,
                    placement_top: 0,
                    width: 0,
                    height: 0,
                    advance,
                    data: Vec::new(),
                }
            };
            self.cache.borrow_mut().insert(key, cached);
        }
        std::cell::Ref::map(self.cache.borrow(), |c| c.get(&key).unwrap())
    }

    pub fn render(
        &self,
        query: &str,
        results: &[crate::search::LauncherItem],
        selected: usize,
        visible: usize,
        cursor: usize,
    ) -> Vec<u32> {
        let mut buf = vec![BG; (self.width * self.height) as usize];

        let font_size = (FONT_SIZE * self.scale).round();
        let hint_size = (HINT_SIZE * self.scale).round();
        let meta_size = (font_size * 0.78).round();

        let row_h = (ROW_H as f32 * self.scale).round() as u32;
        let input_h = (INPUT_H as f32 * self.scale).round() as u32;
        let pad_x = (PAD_X as f32 * self.scale).round() as u32;
        let text_x = pad_x + (20.0 * self.scale).round() as u32;
        let gap = (8.0 * self.scale).round() as u32;

        // ── Input row ─────────────────────────────────────────────────────
        let input_text_y = input_h.saturating_sub(font_size as u32) / 2;
        let metrics = self.primary.as_ref().metrics(&[]).scale(font_size);
        let ascent = metrics.ascent.round() as u32;
        let descent = metrics.descent.abs().round() as u32;
        let cursor_h = ascent + descent;
        let cursor_w = (1.5 * self.scale).round().max(1.0) as u32;
        if query.is_empty() {
            self.draw_text(
                &mut buf,
                "",
                text_x,
                input_text_y,
                FG_HINT,
                hint_size,
                INPUT_LETTER_SPACING,
            );
            self.draw_rect(&mut buf, text_x, input_text_y, cursor_w, cursor_h, FG);
        } else {
            let before = &query[..cursor];
            let after = &query[cursor..];
            let cx = self.draw_text(
                &mut buf,
                before,
                text_x,
                input_text_y,
                FG,
                font_size,
                INPUT_LETTER_SPACING,
            );
            self.draw_text(
                &mut buf,
                after,
                cx,
                input_text_y,
                FG,
                font_size,
                INPUT_LETTER_SPACING,
            );
            self.draw_rect(&mut buf, cx, input_text_y, cursor_w, cursor_h, FG);
        };
        // ── Separator ─────────────────────────────────────────────────────
        self.draw_rect(&mut buf, 0, input_h, self.width, 1, LINE);

        let max_visible = (self.height.saturating_sub(input_h + 1) / row_h) as usize;

        for (i, item) in results.iter().skip(visible).take(max_visible).enumerate() {
            let row_y = input_h + 1 + i as u32 * row_h;
            let real_index = visible + i;

            if row_y + row_h > self.height {
                break;
            }

            if real_index == selected {
                self.draw_rect(&mut buf, 0, row_y, self.width, row_h, SEL_BG);
            }

            let name_y = row_y + (4.0 * self.scale).round() as u32;
            let name_end = self.draw_text(&mut buf, &item.name, pad_x, name_y, FG, font_size, 0.0);

            if item.entry.command != item.name {
                let cmd = format!("  {}", item.entry.command);
                self.draw_text(&mut buf, &cmd, name_end + 4, name_y, FG_DIM, font_size, 0.0);
            }
            let meta_y = name_y + font_size as u32 + (4.0 * self.scale).round() as u32;
            let mut mx = pad_x;

            for tag in &item.entry.tag {
                let label = format!("#{} ", tag);
                mx = self.draw_text(&mut buf, &label, mx, meta_y, FG_DIM, meta_size, 0.0);
                mx += gap;
            }

            let _ = mx;
            if i + 1 < max_visible {
                self.draw_rect(&mut buf, 0, row_y + row_h - 1, self.width, 1, 0x10C0CAF5);
            }
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
        letter_spacing: f32,
    ) -> u32 {
        let size = size.round();
        let ascent = self.primary.as_ref().metrics(&[]).scale(size).ascent;
        let mut cx = x as i32;
        let [_, fg_r, fg_g, fg_b] = color.to_be_bytes();

        for ch in text.chars() {
            if ch.is_control() {
                if ch == '\t' {
                    let space_glyph = self.rasterize_glyph(' ', size);
                    cx += space_glyph.advance as i32 * 4;
                }
                continue;
            }
            let glyph = self.rasterize_glyph(ch, size);
            let glyph_x = cx + glyph.placement_left;
            let glyph_y = y as i32 + ascent as i32 - glyph.placement_top;

            for gy in 0..glyph.height {
                for gx in 0..glyph.width {
                    let coverage = glyph.data[(gy * glyph.width + gx) as usize];
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
            cx += glyph.advance as i32 + (letter_spacing * self.scale).round() as i32;
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
