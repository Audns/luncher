use crate::{
    config::{Config, Scripts},
    renderer::Renderer,
    search::{FuzzySearch, LauncherItem},
    stdin,
};
use calloop::LoopHandle;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keymap, Modifiers, RawModifiers, RepeatInfo},
    },
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
        },
    },
    shm::{Shm, ShmHandler, slot::SlotPool},
};
use wayland_client::{
    Connection, Dispatch, QueueHandle,
    globals::GlobalList,
    protocol::{wl_keyboard::WlKeyboard, wl_shm},
};
use wayland_protocols::wp::viewporter::client::{
    wp_viewport::WpViewport, wp_viewporter::WpViewporter,
};
use xkbcommon::xkb::Keysym;

pub struct AppState {
    pub exit: bool,
    pub width: u32,
    pub height: u32,
    pub configured: bool,
    pub needs_redraw: bool,
    pub registry_state: RegistryState,
    pub seat_state: SeatState,
    pub output_state: OutputState,
    pub shm: Shm,
    pub layer_surface: LayerSurface,
    pub pool: SlotPool,
    pub viewport: Option<WpViewport>,
    pub keyboard: Option<WlKeyboard>,
    pub modifiers: Modifiers,
    pub search: FuzzySearch,
    pub query: String,
    pub selected: usize,
    pub qh: QueueHandle<AppState>,
    pub renderer: Renderer,
    pub loop_handle: LoopHandle<'static, AppState>,
    pub visible: usize,
    pub dmenu_mode: bool,
    pub cursor: usize,
}

impl AppState {
    pub fn new(
        globals: &GlobalList,
        qh: &QueueHandle<Self>,
        loop_handle: LoopHandle<'static, AppState>,
    ) -> Self {
        let cfg = Config::load();
        let scale = cfg.scale;
        let logical_width = cfg.window.width;
        let logical_height = cfg.window.height;
        let phys_w = (logical_width as f32 * scale).round() as u32;
        let phys_h = (logical_height as f32 * scale).round() as u32;

        let compositor_state = CompositorState::bind(globals, qh).unwrap();
        let layer_shell = LayerShell::bind(globals, qh).unwrap();
        let shm = Shm::bind(globals, qh).unwrap();

        let viewporter: Option<WpViewporter> =
            globals.bind::<WpViewporter, _, _>(qh, 1..=1, ()).ok();

        let surface = compositor_state.create_surface(qh);
        let layer_surface =
            layer_shell.create_layer_surface(qh, surface, Layer::Overlay, Some("luncher"), None);
        layer_surface.set_anchor(Anchor::empty());
        layer_surface.set_size(logical_width, logical_height);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        layer_surface.set_exclusive_zone(-1);

        let viewport: Option<WpViewport> = viewporter.as_ref().map(|vp| {
            let v = vp.get_viewport(layer_surface.wl_surface(), qh, ());
            v.set_destination(logical_width as i32, logical_height as i32);
            v
        });

        layer_surface.commit();

        let pool = SlotPool::new((phys_w * phys_h * 4) as usize, &shm).unwrap();

        let (items, dmenu_mode) = if let Some(stdin_items) = stdin::read_stdin() {
            (stdin_items, true)
        } else {
            let scripts = Scripts::load();
            let items: Vec<LauncherItem> = scripts
                .entries
                .into_iter()
                .map(|(name, entry)| LauncherItem::new(name, entry))
                .collect();
            (items, false)
        };

        let mut search = FuzzySearch::new(items);
        search.update("");

        let renderer = Renderer::new(phys_w, phys_h, scale);
        let visible = renderer.max_visible_rows as usize;

        Self {
            exit: false,
            width: phys_w,
            height: phys_h,
            configured: false,
            needs_redraw: true,
            registry_state: RegistryState::new(globals),
            seat_state: SeatState::new(globals, qh),
            output_state: OutputState::new(globals, qh),
            shm,
            layer_surface,
            pool,
            viewport,
            keyboard: None,
            modifiers: Modifiers::default(),
            search,
            query: String::new(),
            selected: 0,
            qh: qh.clone(),
            renderer: renderer,
            loop_handle,
            visible,
            dmenu_mode,
            cursor: 0,
        }
    }

    pub fn draw(&mut self, _qh: &QueueHandle<Self>) {
        self.update_scroll();

        let pixels = self.renderer.render(
            &self.query,
            &self.search.results,
            self.selected,
            self.visible,
            self.cursor,
        );

        let expected = (self.width * self.height * 4) as usize;
        if self.pool.len() < expected {
            self.pool = SlotPool::new(expected, &self.shm).unwrap();
        }
        let (buffer, canvas) = self
            .pool
            .create_buffer(
                self.width as i32,
                self.height as i32,
                (self.width * 4) as i32,
                wl_shm::Format::Argb8888,
            )
            .unwrap();
        let bytes: &[u8] = bytemuck::cast_slice(&pixels);
        if canvas.len() != bytes.len() {
            eprintln!(
                "[draw] size mismatch: canvas={} pixels={}",
                canvas.len(),
                bytes.len()
            );
            return;
        }
        canvas.copy_from_slice(bytes);
        self.layer_surface
            .wl_surface()
            .attach(Some(buffer.wl_buffer()), 0, 0);
        self.layer_surface
            .wl_surface()
            .damage_buffer(0, 0, self.width as i32, self.height as i32);
        self.layer_surface.wl_surface().commit();
        self.needs_redraw = false;
    }

    fn update_scroll(&mut self) {
        let max_visible = self.renderer.max_visible_rows as usize;

        if self.search.results.is_empty() {
            self.visible = 0;
            self.selected = 0;
            return;
        }

        if self.selected < self.visible {
            self.visible = self.selected;
        } else if self.selected >= self.visible + max_visible {
            self.visible = self.selected.saturating_sub(max_visible) + 1;
        }

        let total = self.search.results.len();
        if self.visible > total.saturating_sub(max_visible) {
            self.visible = total.saturating_sub(max_visible);
        }
        if self.selected >= total {
            self.selected = total.saturating_sub(1);
        }
    }

    fn handle_key(&mut self, event: KeyEvent) {
        let ctrl = self.modifiers.ctrl;
        let alt = self.modifiers.alt;

        match event.keysym {
            // ── Exit ──────────────────────────────────────────────────────
            Keysym::Escape => self.exit = true,
            Keysym::c if ctrl => self.exit = true,

            // ── Execute ───────────────────────────────────────────────────
            Keysym::Return | Keysym::KP_Enter => {
                if let Some(item) = self.search.results.get(self.selected) {
                    if self.dmenu_mode {
                        crate::executor::print_selection(&item.entry.command);
                    } else {
                        crate::executor::execute(&item.entry.command);
                    }
                }
                self.exit = true;
            }

            // ── Result navigation ─────────────────────────────────────────
            Keysym::Up => {
                self.selected = self.selected.saturating_sub(1);
                if self.selected < self.visible {
                    self.visible = self.selected;
                }
                self.needs_redraw = true;
            }
            Keysym::Down => {
                let max = self.search.results.len().saturating_sub(1);
                self.selected = (self.selected + 1).min(max);
                let page = self.renderer.max_visible_rows as usize;
                if self.selected >= self.visible + page {
                    self.visible = self.selected + 1 - page;
                }
                self.needs_redraw = true;
            }
            Keysym::p if ctrl => {
                self.selected = self.selected.saturating_sub(1);
                if self.selected < self.visible {
                    self.visible = self.selected;
                }
                self.needs_redraw = true;
            }
            Keysym::n if ctrl => {
                let max = self.search.results.len().saturating_sub(1);
                let page = self.renderer.max_visible_rows as usize;
                self.selected = (self.selected + 1).min(max);
                if self.selected >= self.visible + page {
                    self.visible = self.selected + 1 - page;
                }
                self.needs_redraw = true;
            }
            Keysym::Page_Up => {
                let page = self.renderer.max_visible_rows as usize;
                self.selected = self.selected.saturating_sub(page);
                self.visible = self.visible.saturating_sub(page);
                self.needs_redraw = true;
            }
            Keysym::Page_Down => {
                let page = self.renderer.max_visible_rows as usize;
                let max_sel = self.search.results.len().saturating_sub(1);
                self.selected = (self.selected + page).min(max_sel);
                let max_vis = max_sel.saturating_sub(page.saturating_sub(1));
                self.visible = (self.visible + page).min(max_vis);
                self.needs_redraw = true;
            }
            Keysym::Left if ctrl => {
                self.cursor = prev_word_boundary(&self.query, self.cursor);
                self.needs_redraw = true;
            }
            Keysym::Right if ctrl => {
                self.cursor = next_word_boundary(&self.query, self.cursor);
                self.needs_redraw = true;
            }
            Keysym::Left => {
                self.cursor = prev_char_boundary(&self.query, self.cursor);
                self.needs_redraw = true;
            }
            Keysym::Right => {
                self.cursor = next_char_boundary(&self.query, self.cursor);
                self.needs_redraw = true;
            }
            Keysym::Home => {
                self.cursor = 0;
                self.needs_redraw = true;
            }
            Keysym::End => {
                self.cursor = self.query.len();
                self.needs_redraw = true;
            }
            Keysym::a if ctrl => {
                self.cursor = 0;
                self.needs_redraw = true;
            }
            Keysym::e if ctrl => {
                self.cursor = self.query.len();
                self.needs_redraw = true;
            }
            Keysym::u if ctrl => {
                self.selected = self.selected.saturating_sub(1);
                self.needs_redraw = true;
            }
            Keysym::d if ctrl => {
                let max = self.search.results.len().saturating_sub(1);
                self.selected = (self.selected + 1).min(max);
                self.needs_redraw = true;
            }
            Keysym::k if ctrl => {
                if self.cursor < self.query.len() {
                    self.query.truncate(self.cursor);
                    self.selected = 0;
                    self.search.update(&self.query);
                    self.needs_redraw = true;
                }
            }
            Keysym::BackSpace if ctrl || alt => {
                let new_pos = prev_word_boundary(&self.query, self.cursor);
                self.query.drain(new_pos..self.cursor);
                self.cursor = new_pos;
                self.selected = 0;
                self.search.update(&self.query);
                self.needs_redraw = true;
            }
            Keysym::BackSpace => {
                if self.cursor > 0 {
                    let new_pos = prev_char_boundary(&self.query, self.cursor);
                    self.query.drain(new_pos..self.cursor);
                    self.cursor = new_pos;
                    self.selected = 0;
                    self.search.update(&self.query);
                    self.needs_redraw = true;
                }
            }
            Keysym::Delete => {
                if self.cursor < self.query.len() {
                    let next = next_char_boundary(&self.query, self.cursor);
                    self.query.drain(self.cursor..next);
                    self.selected = 0;
                    self.search.update(&self.query);
                    self.needs_redraw = true;
                }
            }
            _ => {
                if let Some(ch) = event.utf8.and_then(|s| {
                    let mut chars = s.chars();
                    let c = chars.next();
                    if chars.next().is_none() { c } else { None }
                }) {
                    if !ctrl && !alt && !ch.is_control() {
                        self.query.insert(self.cursor, ch);
                        self.cursor += ch.len_utf8();
                        self.selected = 0;
                        self.search.update(&self.query);
                        self.needs_redraw = true;
                    }
                }
            }
        }
    }
}

impl LayerShellHandler for AppState {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &LayerSurface,
        configure: smithay_client_toolkit::shell::wlr_layer::LayerSurfaceConfigure,
        _: u32,
    ) {
        if self.viewport.is_none() {
            if configure.new_size.0 != 0 {
                self.width = configure.new_size.0;
            }
            if configure.new_size.1 != 0 {
                self.height = configure.new_size.1;
            }
        }

        if !self.configured {
            self.configured = true;
            self.draw(qh);
        }
    }
}

impl CompositorHandler for AppState {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wayland_client::protocol::wl_surface::WlSurface,
        _: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wayland_client::protocol::wl_surface::WlSurface,
        _: wayland_client::protocol::wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &wayland_client::protocol::wl_surface::WlSurface,
        _: u32,
    ) {
        if self.needs_redraw {
            self.draw(qh);
        }
    }

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wayland_client::protocol::wl_surface::WlSurface,
        _: &wayland_client::protocol::wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wayland_client::protocol::wl_surface::WlSurface,
        _: &wayland_client::protocol::wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for AppState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wayland_client::protocol::wl_output::WlOutput,
    ) {
    }
    fn update_output(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wayland_client::protocol::wl_output::WlOutput,
    ) {
    }
    fn output_destroyed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wayland_client::protocol::wl_output::WlOutput,
    ) {
    }
}

impl SeatHandler for AppState {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wayland_client::protocol::wl_seat::WlSeat,
    ) {
    }

    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wayland_client::protocol::wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            let keyboard = self
                .seat_state
                .get_keyboard_with_repeat(
                    qh,
                    &seat,
                    None,
                    self.loop_handle.clone(),
                    Box::new(|state: &mut AppState, _qh, event| {
                        state.handle_key(event);
                    }),
                )
                .unwrap();
            self.keyboard = Some(keyboard);
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wayland_client::protocol::wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            if let Some(kbd) = self.keyboard.take() {
                kbd.release();
            }
        }
    }

    fn remove_seat(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wayland_client::protocol::wl_seat::WlSeat,
    ) {
    }
}

impl KeyboardHandler for AppState {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlKeyboard,
        _: &wayland_client::protocol::wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlKeyboard,
        _: &wayland_client::protocol::wl_surface::WlSurface,
        _: u32,
    ) {
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.handle_key(event);
    }

    fn repeat_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.handle_key(event);
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlKeyboard,
        _: u32,
        modifiers: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
        self.modifiers = modifiers;
    }

    fn update_repeat_info(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlKeyboard,
        _: RepeatInfo,
    ) {
    }

    fn update_keymap(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlKeyboard,
        _: Keymap<'_>,
    ) {
    }
}

impl ShmHandler for AppState {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for AppState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

impl Dispatch<WpViewport, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &WpViewport,
        _: wayland_protocols::wp::viewporter::client::wp_viewport::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WpViewporter, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &WpViewporter,
        _: wayland_protocols::wp::viewporter::client::wp_viewporter::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

fn prev_char_boundary(s: &str, pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    let mut i = pos - 1;
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn next_char_boundary(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    let mut i = pos + 1;
    while !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

fn prev_word_boundary(s: &str, pos: usize) -> usize {
    let bytes = s.as_bytes();
    let mut i = pos;
    while i > 0 && bytes[i - 1] == b' ' {
        i -= 1;
    }
    while i > 0 && bytes[i - 1] != b' ' {
        i -= 1;
    }
    i
}

fn next_word_boundary(s: &str, pos: usize) -> usize {
    let bytes = s.as_bytes();
    let mut i = pos;
    while i < s.len() && bytes[i] != b' ' {
        i += 1;
    }
    while i < s.len() && bytes[i] == b' ' {
        i += 1;
    }
    i
}
