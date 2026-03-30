use std::io::Read;
use std::os::fd::{AsFd, FromRawFd, IntoRawFd, OwnedFd};

use anyhow::{Context, Result};
use bytes::Bytes;
use tracing::{info, warn};
use wayland_client::{
    protocol::{wl_registry, wl_seat},
    Connection, Dispatch, EventQueue, QueueHandle,
};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1, ext_data_control_manager_v1, ext_data_control_offer_v1,
};

use crate::clipboard::models::ClipboardEntry;
use crate::clipboard::store::SharedStore;

const SENSITIVE_MIMES: &[&str] = &[
    "x-kde-passwordManagerHint",
    "application/x-kde-passwordmanager",
    "org.freedesktop.secret",
];

pub fn spawn_watcher(store: SharedStore) {
    std::thread::Builder::new()
        .name("clipboard-watcher".into())
        .spawn(move || {
            if let Err(err) = run_watcher(store) {
                eprintln!("[daemon] clipboard watcher crashed: {err:#}");
            }
        })
        .expect("failed to spawn watcher thread");
}

fn run_watcher(store: SharedStore) -> Result<()> {
    let conn = Connection::connect_to_env()
        .context("connecting to Wayland display — is WAYLAND_DISPLAY set?")?;
    let display = conn.display();

    let mut queue: EventQueue<State> = conn.new_event_queue();
    let qh = queue.handle();

    let mut state = State {
        store,
        seat: None,
        manager: None,
        device_created: false,
        pending: None,
    };

    display.get_registry(&qh, ());
    queue
        .roundtrip(&mut state)
        .context("initial Wayland roundtrip")?;

    if state.manager.is_none() {
        anyhow::bail!("compositor does not support ext_data_control_manager_v1");
    }

    info!("watching clipboard");

    loop {
        queue
            .blocking_dispatch(&mut state)
            .context("Wayland event dispatch")?;
    }
}

struct State {
    store: SharedStore,
    seat: Option<wl_seat::WlSeat>,
    manager: Option<ext_data_control_manager_v1::ExtDataControlManagerV1>,
    device_created: bool,
    pending: Option<PendingOffer>,
}

struct PendingOffer {
    proxy: ext_data_control_offer_v1::ExtDataControlOfferV1,
    mimes: Vec<String>,
    sensitive: bool,
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            return;
        };

        match interface.as_str() {
            "wl_seat" => {
                let seat = registry.bind::<wl_seat::WlSeat, _, _>(name, version.min(7), qh, ());
                state.seat = Some(seat);
            }
            "ext_data_control_manager_v1" => {
                let manager = registry
                    .bind::<ext_data_control_manager_v1::ExtDataControlManagerV1, _, _>(
                        name,
                        version.min(1),
                        qh,
                        (),
                    );
                state.manager = Some(manager);
            }
            _ => return,
        }

        if !state.device_created {
            if let (Some(seat), Some(manager)) = (&state.seat, &state.manager) {
                manager.get_data_device(seat, qh, ());
                state.device_created = true;
            }
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(
        _state: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ext_data_control_manager_v1::ExtDataControlManagerV1, ()> for State {
    fn event(
        _state: &mut Self,
        _: &ext_data_control_manager_v1::ExtDataControlManagerV1,
        _: ext_data_control_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ext_data_control_device_v1::ExtDataControlDeviceV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ext_data_control_device_v1::ExtDataControlDeviceV1,
        event: ext_data_control_device_v1::Event,
        _: &(),
        conn: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use ext_data_control_device_v1::Event;

        match event {
            Event::DataOffer { id } => {
                state.pending = Some(PendingOffer {
                    proxy: id,
                    mimes: Vec::new(),
                    sensitive: false,
                });
            }
            Event::Selection { id } => {
                let Some(offer_proxy) = id else { return };
                let Some(offer) = state.pending.take() else {
                    return;
                };
                if offer.proxy != offer_proxy {
                    return;
                }
                process_offer(offer, &state.store, conn);
            }
            Event::Finished => warn!("data control device finished"),
            _ => {}
        }
    }

    fn event_created_child(
        opcode: u16,
        qh: &QueueHandle<Self>,
    ) -> std::sync::Arc<dyn wayland_client::backend::ObjectData> {
        match opcode {
            0 => qh.make_data::<ext_data_control_offer_v1::ExtDataControlOfferV1, ()>(()),
            _ => panic!("unexpected child-creating opcode {opcode} on ext_data_control_device_v1"),
        }
    }
}

impl Dispatch<ext_data_control_offer_v1::ExtDataControlOfferV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ext_data_control_offer_v1::ExtDataControlOfferV1,
        event: ext_data_control_offer_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let ext_data_control_offer_v1::Event::Offer { mime_type } = event else {
            return;
        };

        let Some(offer) = &mut state.pending else {
            return;
        };

        if SENSITIVE_MIMES
            .iter()
            .any(|item| *item == mime_type.as_str())
        {
            offer.sensitive = true;
            return;
        }

        offer.mimes.push(mime_type);
    }
}

fn process_offer(offer: PendingOffer, store: &SharedStore, conn: &Connection) {
    let Some(mime) = choose_mime(&offer.mimes) else {
        return;
    };

    let data = match read_pipe(&offer.proxy, &mime, conn) {
        Ok(data) if data.is_empty() => return,
        Ok(data) => data,
        Err(err) => {
            warn!("pipe read failed for MIME {mime}: {err}");
            return;
        }
    };

    let (mime, data, filename) = normalize_entry(mime, data);
    let entry = ClipboardEntry::with_filename(
        mime,
        Bytes::from(data),
        offer.sensitive,
        Bytes::new(),
        filename,
    );

    if let Err(err) = store.insert(&entry) {
        warn!("store insert error: {err}");
    }
}

fn normalize_entry(mime: String, data: Vec<u8>) -> (String, Vec<u8>, Option<String>) {
    let text_content = String::from_utf8_lossy(&data);
    let is_file_uri = text_content.starts_with("file://");
    let is_absolute_path =
        text_content.starts_with('/') && !text_content.contains('\n') && text_content.len() < 4096;

    if mime == "text/uri-list" || is_file_uri || is_absolute_path {
        let uri_data = if is_absolute_path {
            format!("file://{}", text_content.trim()).into_bytes()
        } else {
            data.clone()
        };

        return match uri_list_filename(&uri_data) {
            Ok(name) => ("text/uri-list".to_string(), uri_data, Some(name)),
            Err(_) => (mime, data, None),
        };
    }

    (mime, data, None)
}

fn choose_mime(mimes: &[String]) -> Option<String> {
    if mimes.iter().any(|mime| mime.as_str() == "text/uri-list") {
        return Some("text/uri-list".to_string());
    }

    const TEXT_PREF: &[&str] = &[
        "text/plain;charset=utf-8",
        "text/plain",
        "text/html",
        "UTF8_STRING",
        "STRING",
        "TEXT",
    ];
    for pref in TEXT_PREF {
        if let Some(mime) = mimes.iter().find(|mime| mime.as_str() == *pref) {
            return Some(mime.clone());
        }
    }

    const IMAGE_PREF: &[&str] = &["image/png", "image/jpeg", "image/webp", "image/gif"];
    for pref in IMAGE_PREF {
        if let Some(mime) = mimes.iter().find(|mime| mime.as_str() == *pref) {
            return Some(mime.clone());
        }
    }
    if let Some(mime) = mimes.iter().find(|mime| mime.starts_with("image/")) {
        return Some(mime.clone());
    }

    let skip_prefixes = [
        "x-special/",
        "application/x-kde-",
        "chromium/",
        "x-moz-",
        "TARGETS",
        "MULTIPLE",
        "SAVE_TARGETS",
        "TIMESTAMP",
        "ATOM",
        "INTEGER",
    ];
    mimes
        .iter()
        .find(|mime| !skip_prefixes.iter().any(|prefix| mime.starts_with(prefix)))
        .cloned()
}

fn read_pipe(
    offer: &ext_data_control_offer_v1::ExtDataControlOfferV1,
    mime: &str,
    conn: &Connection,
) -> Result<Vec<u8>> {
    let (read_sock, write_sock) = std::os::unix::net::UnixStream::pair()
        .context("creating socket pair for clipboard pipe")?;

    let (read_fd, write_fd) = unsafe {
        (
            OwnedFd::from_raw_fd(read_sock.into_raw_fd()),
            OwnedFd::from_raw_fd(write_sock.into_raw_fd()),
        )
    };

    offer.receive(mime.to_string(), write_fd.as_fd());
    conn.flush()
        .context("flushing Wayland connection after receive()")?;
    drop(write_fd);

    let mut file = std::fs::File::from(read_fd);
    let mut buf = Vec::with_capacity(4096);
    file.read_to_end(&mut buf)
        .context("reading clipboard pipe")?;
    while buf.last() == Some(&0) {
        buf.pop();
    }
    Ok(buf)
}

fn uri_list_filename(data: &[u8]) -> Result<String> {
    let uri_str = String::from_utf8_lossy(data);
    let uri = uri_str
        .lines()
        .find(|line| line.starts_with("file://"))
        .ok_or_else(|| anyhow::anyhow!("no file:// URI found"))?;

    let path = uri
        .strip_prefix("file://")
        .ok_or_else(|| anyhow::anyhow!("invalid file:// URI"))?;

    let decoded = if path.starts_with('/') {
        percent_decode(path)
    } else {
        path.to_string()
    };

    std::path::Path::new(&decoded)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .context("file uri has no filename")
}

fn percent_decode(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2 {
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    out.push(byte as char);
                    continue;
                }
            }
            out.push('%');
            out.push_str(&hex);
        } else {
            out.push(ch);
        }
    }
    out
}
