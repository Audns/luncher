# Configuration Example

Create a file at `~/.config/luncher/config.toml`. All keys are optional—omitted values fall back to the defaults shown below.

## Full Example

```toml
scale = 1.0
single_instance = true
case_sensitive = false

[window]
width = 1200
height = 800

[theme]
bg = 0xFF1E1E2E
fg = 0xE6E6E6FF
fg_dim = 0x73C0CAF5
fg_hint = 0x4DC0CAF5
sel_bg = 0x15C0CAF5
line = 0xFF2A2A3E

[layout]
font_size = 22.0
hint_size = 22.0
row_h = 58
input_h = 45
pad_x = 16
input_letter_spacing = 0.5

[clipboard]
history_limit = 50

[font]
primary = ["/usr/share/fonts/noto/NotoSans-Regular.ttf"]
fallback = ["/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf"]
emoji = ["/usr/share/fonts/noto/NotoColorEmoji.ttf"]
cjk = [
  "/usr/share/fonts/adobe-source-han-sans/SourceHanSansCN-Regular.otf",
  "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
]
```

## Options

### `scale`

- **Type:** `f32`
- **Default:** `1.0`
- UI scale factor. Higher values make everything larger.

### `single_instance`

- **Type:** `bool`
- **Default:** `true`
- Whether only one instance of luncher should run at a time.

### `case_sensitive`

- **Type:** `bool`
- **Default:** `false`
- Enable case-sensitive fuzzy search.

### `[window]`

#### `width`

- **Type:** `u32`
- **Default:** `1200`
- Window width in logical pixels.

#### `height`

- **Type:** `u32`
- **Default:** `800`
- Window height in logical pixels.

### `[theme]` — Colors

Colors are specified as `0xAARRGGBB` hex integers.

| Key | Default | Description |
|---|---|---|
| `bg` | `0xFF1E1E2E` | Window background color |
| `fg` | `0xE6E6E6FF` | Primary text color |
| `fg_dim` | `0x73C0CAF5` | Dimmed text (meta, tags, scrollbar thumb) |
| `fg_hint` | `0x4DC0CAF5` | Input placeholder / empty hint color |
| `sel_bg` | `0x15C0CAF5` | Selected row background |
| `line` | `0xFF2A2A3E` | Separator line color |

### `[layout]` — Sizing & Spacing

| Key | Type | Default | Description |
|---|---|---|---|
| `font_size` | `f32` | `22.0` | Base font size for item names and input text |
| `hint_size` | `f32` | `22.0` | Font size for mode prefix hints |
| `row_h` | `u32` | `58` | Height of each result row in pixels |
| `input_h` | `u32` | `45` | Height of the search input area |
| `pad_x` | `u32` | `16` | Horizontal padding inside the window |
| `input_letter_spacing` | `f32` | `0.5` | Extra spacing between typed characters |

### `[clipboard]`

| Key | Type | Default | Description |
|---|---|---|---|
| `history_limit` | `usize` | `50` | Maximum number of clipboard entries the daemon holds in memory and shows in `luncher -m clipboard`. Lowering this only affects what the UI displays — the on-disk `SQLite` history is still trimmed to a hard cap of `1000` entries. |

### `[font]`

Each field is a list of `.ttf` / `.otf` / `.ttc` paths. The renderer tries
them in order and uses the first one that opens. If every configured path
fails, the built-in defaults are tried as a fallback. The primary font is
required — startup panics if no candidate opens.

| Key | Type | Default | Description |
|---|---|---|---|
| `primary` | `[String]` | `["/usr/share/fonts/noto/NotoSans-Regular.ttf"]` | Main text font. |
| `fallback` | `[String]` | `["/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf"]` | Used for glyphs missing in the primary font (and the emoji font). |
| `emoji` | `[String]` | `["/usr/share/fonts/noto/NotoColorEmoji.ttf"]` | Color-emoji font, tried between primary and fallback. |
| `cjk` | `[String]` | Source Han Sans CN, then Noto Sans CJK | Tried last for CJK code points. |
