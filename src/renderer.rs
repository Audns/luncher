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
const HINT_SIZE: f32 = 25.0;
const ROW_H: u32 = 35;
const INPUT_H: u32 = 45;
const PAD_X: u32 = 16;

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

/// Owned font data backed by a memory-mapped file.
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

    /// Extend the lifetime of the mmap slice for initial parsing only.
    /// SAFETY: caller must ensure the Mmap outlives the returned reference.
    unsafe fn mmap_as_static(mmap: &Mmap) -> &'static [u8] {
        unsafe { std::mem::transmute::<&[u8], &'static [u8]>(mmap.as_ref()) }
    }
}

/// Cached rasterized glyph bitmap.
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

        let max_visible_rows: u32 = {
            let row_h = (ROW_H as f32 * scale).round() as u32;
            let input_h = (INPUT_H as f32 * scale).round() as u32;
            (height.saturating_sub(input_h + 1) / row_h) as u32
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

    /// Pick the font that has a glyph for `ch`, returning (FontRef, GlyphId).
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

    /// Rasterize a glyph (or return from cache).
    fn rasterize_glyph(&self, ch: char, size: f32) -> std::cell::Ref<'_, CachedGlyph> {
        let size_key = size.to_bits();
        let key = (ch, size_key);

        {
            let has = self.cache.borrow().contains_key(&key);
            if !has {
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
        }

        std::cell::Ref::map(self.cache.borrow(), |c| c.get(&key).unwrap())
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
        let ascent = {
            let font_ref = self.primary.as_ref();
            font_ref.metrics(&[]).scale(size).ascent
        };

        let mut cx = x as i32;
        let [_, fg_r, fg_g, fg_b] = color.to_be_bytes();

        for ch in text.chars() {
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

            cx += glyph.advance as i32;
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
