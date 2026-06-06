use std::sync::{Arc, Mutex};

use crate::ecs::{FetchState, KhronosAsset, PendingAsset, PolyAsset, ViewerWorld};
use nightshade::prelude::{ehttp, serde_json};
use protocol::{AssetKind, KhronosEntry, PolyhavenEntry, WorkerMessage};
use serde::Deserialize;

const KHRONOS_BASE: &str =
    "https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Assets/main/Models";
const KHRONOS_INDEX: &str = "https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Assets/main/Models/model-index.json";
const POLYHAVEN_ASSETS: &str = "https://api.polyhaven.com/assets?type=hdris";
const POLYHAVEN_FILES: &str = "https://api.polyhaven.com/files/";
const POLYHAVEN_THUMB: &str = "https://cdn.polyhaven.com/asset_img/thumbs/";

#[derive(Deserialize)]
struct KhronosRaw {
    label: String,
    name: String,
    #[serde(default)]
    variants: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    screenshot: Option<String>,
}

#[derive(Deserialize)]
struct PolyRaw {
    name: String,
}

#[derive(Deserialize)]
struct FileLink {
    url: String,
}

#[derive(Deserialize)]
struct HdriResolution {
    #[serde(default)]
    hdr: Option<FileLink>,
}

#[derive(Deserialize)]
struct HdriFiles {
    hdri: std::collections::BTreeMap<String, HdriResolution>,
}

/// Kicks off the index fetches if they have not started.
pub fn ensure_indices(viewer: &ViewerWorld) {
    ensure_khronos(&viewer.resources.browsers.khronos);
    ensure_polyhaven(&viewer.resources.browsers.polyhaven);
}

/// Streams each browser list to the page once its index has loaded.
pub fn poll(viewer: &mut ViewerWorld) {
    if !viewer.resources.browsers.khronos_sent
        && let Ok(guard) = viewer.resources.browsers.khronos.lock()
        && let FetchState::Loaded(entries) = &*guard
    {
        let list = entries
            .iter()
            .map(|entry| KhronosEntry {
                name: entry.name.clone(),
                label: entry.label.clone(),
                thumbnail: entry.thumbnail.clone(),
            })
            .collect();
        crate::post(&WorkerMessage::KhronosList { entries: list });
        drop(guard);
        viewer.resources.browsers.khronos_sent = true;
    }

    if !viewer.resources.browsers.polyhaven_sent
        && let Ok(guard) = viewer.resources.browsers.polyhaven.lock()
        && let FetchState::Loaded(entries) = &*guard
    {
        let list = entries
            .iter()
            .map(|entry| PolyhavenEntry {
                slug: entry.slug.clone(),
                name: entry.name.clone(),
                thumbnail: entry.thumbnail.clone(),
            })
            .collect();
        crate::post(&WorkerMessage::PolyhavenList { entries: list });
        drop(guard);
        viewer.resources.browsers.polyhaven_sent = true;
    }
}

/// Re-sends the loaded lists (used when the page UI re-requests them).
pub fn resend(viewer: &mut ViewerWorld) {
    viewer.resources.browsers.khronos_sent = false;
    viewer.resources.browsers.polyhaven_sent = false;
}

/// Fetches a Khronos sample model by name into the asset inbox.
pub fn fetch_khronos(viewer: &ViewerWorld, name: &str) {
    let url = {
        let Ok(guard) = viewer.resources.browsers.khronos.lock() else {
            return;
        };
        let FetchState::Loaded(entries) = &*guard else {
            return;
        };
        match entries
            .iter()
            .find(|entry| entry.name == name)
            .and_then(|entry| entry.glb_url.clone())
        {
            Some(url) => url,
            None => return,
        }
    };
    if !begin_loading(viewer, name) {
        return;
    }
    download_into(
        url,
        AssetKind::Model,
        Arc::clone(&viewer.resources.incoming.asset),
        Arc::clone(&viewer.resources.incoming.loading),
    );
}

/// Fetches a Polyhaven HDRI by slug: resolve the file list, then download.
pub fn fetch_polyhaven(viewer: &ViewerWorld, slug: &str) {
    if !begin_loading(viewer, slug) {
        return;
    }
    let files_url = format!("{POLYHAVEN_FILES}{slug}");
    let asset = Arc::clone(&viewer.resources.incoming.asset);
    let loading = Arc::clone(&viewer.resources.incoming.loading);
    ehttp::fetch(ehttp::Request::get(&files_url), move |result| {
        let url = result
            .ok()
            .filter(|response| response.ok)
            .and_then(|response| serde_json::from_slice::<HdriFiles>(&response.bytes).ok())
            .and_then(pick_hdr);
        match url {
            Some(url) => download_into(url, AssetKind::Hdri, asset, loading),
            None => {
                if let Ok(mut guard) = loading.lock() {
                    *guard = None;
                }
            }
        }
    });
}

fn begin_loading(viewer: &ViewerWorld, label: &str) -> bool {
    let Ok(mut guard) = viewer.resources.incoming.loading.lock() else {
        return false;
    };
    if guard.is_some() {
        return false;
    }
    *guard = Some(label.to_string());
    crate::post(&WorkerMessage::Loading {
        active: true,
        label: label.to_string(),
    });
    true
}

fn download_into(
    url: String,
    kind: AssetKind,
    asset: Arc<Mutex<Option<PendingAsset>>>,
    loading: Arc<Mutex<Option<String>>>,
) {
    ehttp::fetch(ehttp::Request::get(&url), move |result| {
        if let Ok(response) = result
            && response.ok
            && let Ok(mut slot) = asset.lock()
        {
            *slot = Some(PendingAsset {
                kind,
                bytes: response.bytes,
            });
        }
        if let Ok(mut guard) = loading.lock() {
            *guard = None;
        }
    });
}

fn ensure_khronos(state: &Arc<Mutex<FetchState<Vec<KhronosAsset>>>>) {
    {
        let Ok(guard) = state.lock() else {
            return;
        };
        if !matches!(*guard, FetchState::Idle) {
            return;
        }
    }
    *state.lock().unwrap() = FetchState::Loading;
    let target = Arc::clone(state);
    ehttp::fetch(ehttp::Request::get(KHRONOS_INDEX), move |result| {
        let next = match result {
            Ok(response) if response.ok => {
                match serde_json::from_slice::<Vec<KhronosRaw>>(&response.bytes) {
                    Ok(raw) => FetchState::Loaded(khronos_entries(raw)),
                    Err(_) => FetchState::Failed,
                }
            }
            Ok(_) => FetchState::Failed,
            Err(_) => FetchState::Failed,
        };
        if let Ok(mut guard) = target.lock() {
            *guard = next;
        }
    });
}

fn khronos_entries(raw: Vec<KhronosRaw>) -> Vec<KhronosAsset> {
    let mut entries: Vec<KhronosAsset> = raw
        .into_iter()
        .map(|entry| {
            let glb_url = entry
                .variants
                .get("glTF-Binary")
                .map(|file| format!("{KHRONOS_BASE}/{}/glTF-Binary/{file}", entry.name));
            let thumbnail = entry
                .screenshot
                .as_ref()
                .map(|path| format!("{KHRONOS_BASE}/{}/{path}", entry.name));
            KhronosAsset {
                name: entry.name,
                label: entry.label,
                glb_url,
                thumbnail,
            }
        })
        .collect();
    entries.retain(|entry| entry.glb_url.is_some());
    entries.sort_by_key(|entry| entry.label.to_lowercase());
    entries
}

fn ensure_polyhaven(state: &Arc<Mutex<FetchState<Vec<PolyAsset>>>>) {
    {
        let Ok(guard) = state.lock() else {
            return;
        };
        if !matches!(*guard, FetchState::Idle) {
            return;
        }
    }
    *state.lock().unwrap() = FetchState::Loading;
    let target = Arc::clone(state);
    ehttp::fetch(ehttp::Request::get(POLYHAVEN_ASSETS), move |result| {
        let next = match result {
            Ok(response) if response.ok => match serde_json::from_slice::<
                std::collections::BTreeMap<String, PolyRaw>,
            >(&response.bytes)
            {
                Ok(raw) => FetchState::Loaded(poly_entries(raw)),
                Err(_) => FetchState::Failed,
            },
            Ok(_) => FetchState::Failed,
            Err(_) => FetchState::Failed,
        };
        if let Ok(mut guard) = target.lock() {
            *guard = next;
        }
    });
}

fn poly_entries(raw: std::collections::BTreeMap<String, PolyRaw>) -> Vec<PolyAsset> {
    let mut entries: Vec<PolyAsset> = raw
        .into_iter()
        .map(|(slug, asset)| PolyAsset {
            thumbnail: format!("{POLYHAVEN_THUMB}{slug}.png?width=256&height=256"),
            slug,
            name: asset.name,
        })
        .collect();
    entries.sort_by_key(|entry| entry.name.to_lowercase());
    entries
}

fn pick_hdr(files: HdriFiles) -> Option<String> {
    let mut entries: Vec<(u32, String)> = files
        .hdri
        .into_iter()
        .filter_map(|(key, resolution)| {
            resolution
                .hdr
                .map(|link| (resolution_value(&key), link.url))
        })
        .collect();
    entries.sort_by_key(|(value, _)| *value);
    entries
        .iter()
        .find(|(value, _)| *value == 1)
        .map(|(_, url)| url.clone())
        .or_else(|| entries.into_iter().next().map(|(_, url)| url))
}

fn resolution_value(key: &str) -> u32 {
    key.trim_end_matches(['k', 'K']).parse().unwrap_or(u32::MAX)
}
