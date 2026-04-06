use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;

use memmap2::Mmap;
use swash::scale::image::{Content, Image};
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
const INPUT_LETTER_SPACING: f32 = 0.5;

const PRIMARY_FONT_PATHS: &[&str] = &[
    "/usr/share/fonts/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf",
];

const EMOJI_FONT_PATHS: &[&str] = &["/usr/share/fonts/noto/NotoColorEmoji.ttf"];

const CJK_FONT_PATHS: &[&str] = &[
    "/usr/share/fonts/adobe-source-han-sans/SourceHanSansCN-Regular.otf",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
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
    is_color: bool,
}

pub struct Renderer {
    primary: MappedFont,
    fallback: Option<MappedFont>,
    emoji: Option<MappedFont>,
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
        let primary = MappedFont::open(PRIMARY_FONT_PATHS[0])
            .unwrap_or_else(|| panic!("No primary font found at {}", PRIMARY_FONT_PATHS[0]));

        let fallback = if PRIMARY_FONT_PATHS.len() > 1 {
            MappedFont::open(PRIMARY_FONT_PATHS[1])
        } else {
            None
        };

        let emoji = EMOJI_FONT_PATHS.iter().find_map(|p| MappedFont::open(p));
        let cjk = CJK_FONT_PATHS.iter().find_map(|p| MappedFont::open(p));

        let max_visible_rows = {
            let row_h = (ROW_H as f32 * scale).round() as u32;
            let input_h = (INPUT_H as f32 * scale).round() as u32;
            height.saturating_sub(input_h + 1) / row_h
        };

        Self {
            primary,
            fallback,
            emoji,
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
        if let Some(emoji) = &self.emoji {
            let emoji_ref = emoji.as_ref();
            let gid = emoji_ref.charmap().map(ch);
            if gid != 0 {
                return (emoji_ref, gid);
            }
        }
        if let Some(fallback) = &self.fallback {
            let fb_ref = fallback.as_ref();
            let gid = fb_ref.charmap().map(ch);
            if gid != 0 {
                return (fb_ref, gid);
            }
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
                let is_color = img.content == Content::Color;
                CachedGlyph {
                    placement_left: img.placement.left,
                    placement_top: img.placement.top,
                    width: img.placement.width,
                    height: img.placement.height,
                    advance,
                    data: img.data,
                    is_color,
                }
            } else {
                CachedGlyph {
                    placement_left: 0,
                    placement_top: 0,
                    width: 0,
                    height: 0,
                    advance,
                    data: Vec::new(),
                    is_color: false,
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
        mode: &str,
    ) -> Vec<u32> {
        let mut buf = vec![BG; (self.width * self.height) as usize];

        let font_size = (FONT_SIZE * self.scale).round();
        let hint_size = (HINT_SIZE * self.scale).round();
        let meta_size = (font_size * 0.78).round();

        let row_h = (ROW_H as f32 * self.scale).round() as u32;
        let input_h = (INPUT_H as f32 * self.scale).round() as u32;
        let pad_x = (PAD_X as f32 * self.scale).round() as u32;
        let gap = (8.0 * self.scale).round() as u32;

        // ── Input row ─────────────────────────────────────────────────────
        let input_text_y = input_h.saturating_sub(font_size as u32) / 2;

        // ── Mode prefix ─────────────────────────────────────────────────────
        let mode_prefix = format!("{}> ", mode);
        let prefix_w = self.measure_text_width(&mode_prefix, hint_size);
        let prefix_x = pad_x;
        if !mode.is_empty() {
            self.draw_text(
                &mut buf,
                &mode_prefix,
                prefix_x,
                input_text_y,
                FG,
                hint_size,
                INPUT_LETTER_SPACING,
            );
        }
        let text_x = pad_x + prefix_w + (20.0 * self.scale).round() as u32 - 15;

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

            let inline_meta = if let Some(ref meta) = item.entry.inline_meta {
                if meta.is_empty() {
                    None
                } else {
                    Some(meta.as_str())
                }
            } else if !item.entry.name.is_empty() && item.entry.name != item.name {
                Some(item.entry.name.as_str())
            } else if item.entry.command != item.name {
                Some(item.entry.command.as_str())
            } else {
                None
            };

            if let Some(meta) = inline_meta {
                let meta = format!("  {}", meta);
                self.draw_text(
                    &mut buf,
                    &meta,
                    name_end + 4,
                    name_y,
                    FG_DIM,
                    font_size,
                    0.0,
                );
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
                    let idx = (gy * glyph.width + gx) as usize;
                    let (a, fr, fg, fb) = if glyph.is_color {
                        let base = idx * 4;
                        if base + 3 >= glyph.data.len() {
                            continue;
                        }
                        let r = glyph.data[base] as u32;
                        let g = glyph.data[base + 1] as u32;
                        let b = glyph.data[base + 2] as u32;
                        let a = glyph.data[base + 3] as u32;
                        (a, r, g, b)
                    } else {
                        let a = glyph.data[idx] as u32;
                        (a, fg_r as u32, fg_g as u32, fg_b as u32)
                    };
                    if a == 0 {
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

                    let bidx = (py * self.width + px) as usize;
                    let ia = 255 - a;
                    let bg = buf[bidx];
                    let [_, bg_r, bg_g, bg_b] = bg.to_be_bytes();
                    let r = (fr * a + bg_r as u32 * ia) / 255;
                    let g = (fg * a + bg_g as u32 * ia) / 255;
                    let b = (fb * a + bg_b as u32 * ia) / 255;
                    buf[bidx] = 0xFF000000 | (r << 16) | (g << 8) | b;
                }
            }
            cx += glyph.advance as i32 + (letter_spacing * self.scale).round() as i32;
        }
        cx.max(x as i32) as u32
    }

    pub fn render_preview(
        &self,
        item: &crate::search::LauncherItem,
        full_content: Option<&str>,
        scroll_line: usize,
    ) -> (Vec<u32>, usize) {
        let mut buf = vec![BG; (self.width * self.height) as usize];

        let font_size = (FONT_SIZE * self.scale).round();
        let meta_size = (font_size * 0.85).round();
        let pad_x = (PAD_X as f32 * self.scale).round() as u32;
        let pad_y = (20.0 * self.scale).round() as u32;
        let line_height = (ROW_H as f32 * self.scale).round() as u32;
        let content_width = self.width.saturating_sub(pad_x * 2);

        let bar_h = line_height + (8.0 * self.scale).round() as u32;
        let bar_y = self.height.saturating_sub(bar_h);
        let content_area = bar_y.saturating_sub(pad_y);
        let visible_lines = if line_height > 0 {
            (content_area / line_height) as usize
        } else {
            0
        };

        let mut lines: Vec<(String, f32, u32)> = Vec::new();

        if let Some(content) = full_content {
            if !content.is_empty() {
                self.compute_wrapped_lines(
                    &mut lines,
                    content,
                    font_size as f32,
                    content_width,
                    FG,
                );
            }
        } else {
            self.compute_wrapped_lines(&mut lines, &item.name, font_size as f32, content_width, FG);

            let meta_text = if let Some(ref meta) = item.entry.inline_meta {
                if !meta.is_empty() {
                    Some(meta.as_str())
                } else {
                    None
                }
            } else if !item.entry.name.is_empty() && item.entry.name != item.name {
                Some(item.entry.name.as_str())
            } else if item.entry.command != item.name {
                Some(item.entry.command.as_str())
            } else {
                None
            };

            if let Some(meta) = meta_text {
                self.compute_wrapped_lines(
                    &mut lines,
                    meta,
                    meta_size as f32,
                    content_width,
                    FG_DIM,
                );
            }
        }

        let total_lines = lines.len();
        let max_scroll = total_lines.saturating_sub(visible_lines);
        let start = scroll_line.min(max_scroll);
        let end = (start + visible_lines).min(total_lines);
        for (i, idx) in (start..end).enumerate() {
            let (ref text, size, color) = lines[idx];
            let y = pad_y + (i as u32 * line_height);
            if !text.is_empty() {
                self.draw_text(&mut buf, text, pad_x, y, color, size, 0.0);
            }
        }

        self.draw_rect(&mut buf, 0, bar_y.saturating_sub(1), self.width, 1, LINE);

        if !item.entry.tag.is_empty() {
            let tag_y = bar_y + (4.0 * self.scale).round() as u32 + 16;
            let mut x = pad_x;
            let gap = (8.0 * self.scale).round() as u32;
            for tag in &item.entry.tag {
                let label = format!("#{}  ", tag);
                x = self.draw_text(&mut buf, &label, x, tag_y, FG, meta_size, 0.0);
                x += gap;
            }
        }

        if total_lines > visible_lines {
            let scroll_pct = if max_scroll == 0 {
                0.0
            } else {
                scroll_line as f32 / max_scroll as f32
            };
            let thumb_h =
                ((visible_lines as f32 / total_lines as f32) * bar_y as f32).max(4.0) as u32;
            let track_h = bar_y.saturating_sub(thumb_h);
            let thumb_y = ((scroll_pct * track_h as f32) as u32).min(track_h);
            let thumb_x = self.width.saturating_sub((6.0 * self.scale).round() as u32);
            let thumb_w = (3.0 * self.scale).round() as u32;
            self.draw_rect(&mut buf, thumb_x, thumb_y, thumb_w, thumb_h, FG_DIM);
        }

        (buf, max_scroll)
    }

    fn compute_wrapped_lines(
        &self,
        lines: &mut Vec<(String, f32, u32)>,
        text: &str,
        size: f32,
        max_width: u32,
        color: u32,
    ) {
        let size = size.round();
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut line_words: Vec<&str> = Vec::new();

        for word in words {
            line_words.push(word);
            let line_text = line_words.join(" ");
            let test_width = self.measure_text_width(&line_text, size);

            if test_width > max_width && line_words.len() > 1 {
                line_words.pop();
                lines.push((line_words.join(" "), size, color));
                line_words = vec![word];
            }
        }

        if !line_words.is_empty() {
            lines.push((line_words.join(" "), size, color));
        }
    }

    fn measure_text_width(&self, text: &str, size: f32) -> u32 {
        let mut width = 0f32;
        for ch in text.chars() {
            if ch.is_control() {
                continue;
            }
            let glyph = self.rasterize_glyph(ch, size);
            width += glyph.advance;
        }
        width.round() as u32
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
