use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[cfg(feature = "agent")]
use crate::ecs::{AgentModelLoad, AgentModelQueue};
use crate::ecs::{FetchState, KhronosAsset, PendingAsset, PolyAsset, ViewerWorld};
use nightshade::prelude::{ehttp, serde_json};
use protocol::{KhronosEntry, PolyhavenEntry, WorkerMessage};
use serde::Deserialize;

const KHRONOS_BASE: &str =
    "https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Assets/main/Models";
const KHRONOS_INDEX: &str = "https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Assets/main/Models/model-index.json";
const POLYHAVEN_HDRIS: &str = "https://api.polyhaven.com/assets?type=hdris";
const POLYHAVEN_MODELS: &str = "https://api.polyhaven.com/assets?type=models";
const POLYHAVEN_FILES: &str = "https://api.polyhaven.com/files/";
const POLYHAVEN_THUMB: &str = "https://cdn.polyhaven.com/asset_img/thumbs/";

type AssetSlot = Arc<Mutex<Option<PendingAsset>>>;
type LoadingSlot = Arc<Mutex<Option<String>>>;

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

#[derive(Deserialize)]
struct IncludeFile {
    url: String,
}

#[derive(Deserialize)]
struct GltfFile {
    url: String,
    #[serde(default)]
    include: std::collections::BTreeMap<String, IncludeFile>,
}

#[derive(Deserialize)]
struct GltfResolution {
    gltf: GltfFile,
}

#[derive(Deserialize)]
struct ModelFiles {
    gltf: std::collections::BTreeMap<String, GltfResolution>,
}

/// Kicks off the index fetches if they have not started.
pub fn ensure_indices(viewer: &ViewerWorld) {
    ensure_khronos(&viewer.resources.browsers.khronos);
    ensure_polyhaven(&viewer.resources.browsers.hdris, POLYHAVEN_HDRIS);
    ensure_polyhaven(&viewer.resources.browsers.models, POLYHAVEN_MODELS);
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

    if !viewer.resources.browsers.hdris_sent
        && let Some(list) = poly_list(&viewer.resources.browsers.hdris)
    {
        crate::post(&WorkerMessage::PolyhavenList { entries: list });
        viewer.resources.browsers.hdris_sent = true;
    }

    if !viewer.resources.browsers.models_sent
        && let Some(list) = poly_list(&viewer.resources.browsers.models)
    {
        crate::post(&WorkerMessage::PolyhavenModelsList { entries: list });
        viewer.resources.browsers.models_sent = true;
    }
}

fn poly_list(state: &Arc<Mutex<FetchState<Vec<PolyAsset>>>>) -> Option<Vec<PolyhavenEntry>> {
    let guard = state.lock().ok()?;
    let FetchState::Loaded(entries) = &*guard else {
        return None;
    };
    Some(
        entries
            .iter()
            .map(|entry| PolyhavenEntry {
                slug: entry.slug.clone(),
                name: entry.name.clone(),
                thumbnail: entry.thumbnail.clone(),
            })
            .collect(),
    )
}

/// Re-sends the loaded lists (used when the page UI re-requests them).
pub fn resend(viewer: &mut ViewerWorld) {
    viewer.resources.browsers.khronos_sent = false;
    viewer.resources.browsers.hdris_sent = false;
    viewer.resources.browsers.models_sent = false;
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
    let (asset, loading) = slots(viewer);
    download_single(url, PendingAsset::Model, asset, loading);
}

/// Fetches a Polyhaven HDRI by slug at a preferred resolution (in k): resolve
/// the file list, then download.
pub fn fetch_polyhaven(viewer: &ViewerWorld, slug: &str, resolution: u32) {
    if !begin_loading(viewer, slug) {
        return;
    }
    let (asset, loading) = slots(viewer);
    let files_url = format!("{POLYHAVEN_FILES}{slug}");
    ehttp::fetch(ehttp::Request::get(&files_url), move |result| {
        let url = result
            .ok()
            .filter(|response| response.ok)
            .and_then(|response| serde_json::from_slice::<HdriFiles>(&response.bytes).ok())
            .and_then(|files| pick_hdr(files, resolution));
        match url {
            Some(url) => download_single(url, PendingAsset::Hdri, asset, loading),
            None => clear(&loading),
        }
    });
}

/// Fetches a Polyhaven model by slug at a preferred texture resolution (in k):
/// resolve the glTF plus its textures, then download them all into a map.
pub fn fetch_polyhaven_model(viewer: &ViewerWorld, slug: &str, resolution: u32) {
    if !begin_loading(viewer, slug) {
        return;
    }
    let (asset, loading) = slots(viewer);
    let files_url = format!("{POLYHAVEN_FILES}{slug}");
    ehttp::fetch(ehttp::Request::get(&files_url), move |result| {
        let gltf = result
            .ok()
            .filter(|response| response.ok)
            .and_then(|response| serde_json::from_slice::<ModelFiles>(&response.bytes).ok())
            .and_then(|files| pick_model(files, resolution));
        match gltf {
            Some(gltf) => {
                let includes: Vec<(String, String)> = gltf
                    .include
                    .into_iter()
                    .map(|(key, file)| (key, file.url))
                    .collect();
                download_model(gltf.url, includes, asset, loading);
            }
            None => clear(&loading),
        }
    });
}

/// Like `fetch_polyhaven_model`, but the resolved glTF and its resources are
/// pushed onto the agent's additive model queue (tagged with `correlation_id`)
/// instead of the replace inbox, so the model joins the scene without wiping it.
#[cfg(feature = "agent")]
pub fn fetch_polyhaven_model_additive(
    viewer: &ViewerWorld,
    slug: &str,
    resolution: u32,
    correlation_id: u64,
) {
    let queue = Arc::clone(&viewer.resources.incoming.agent_models);
    let files_url = format!("{POLYHAVEN_FILES}{slug}");
    ehttp::fetch(ehttp::Request::get(&files_url), move |result| {
        let gltf = result
            .ok()
            .filter(|response| response.ok)
            .and_then(|response| serde_json::from_slice::<ModelFiles>(&response.bytes).ok())
            .and_then(|files| pick_model(files, resolution));
        match gltf {
            Some(gltf) => {
                let includes: Vec<(String, String)> = gltf
                    .include
                    .into_iter()
                    .map(|(key, file)| (key, file.url))
                    .collect();
                download_model_additive(gltf.url, includes, queue, correlation_id);
            }
            None => crate::agent::fail(correlation_id, "could not resolve the Polyhaven model"),
        }
    });
}

#[cfg(feature = "agent")]
fn download_model_additive(
    gltf_url: String,
    includes: Vec<(String, String)>,
    queue: AgentModelQueue,
    correlation_id: u64,
) {
    ehttp::fetch(ehttp::Request::get(&gltf_url), move |result| {
        let gltf = match result.ok().filter(|response| response.ok).map(|r| r.bytes) {
            Some(bytes) => bytes,
            None => {
                crate::agent::fail(correlation_id, "failed to download the Polyhaven glTF");
                return;
            }
        };
        if includes.is_empty() {
            if let Ok(mut guard) = queue.lock() {
                guard.push(AgentModelLoad {
                    correlation_id,
                    gltf,
                    resources: HashMap::new(),
                });
            }
            return;
        }

        let total = includes.len();
        let progress = Arc::new(Mutex::new((Some(gltf), HashMap::new(), 0usize, false)));
        for (key, url) in includes {
            let progress = Arc::clone(&progress);
            let queue = Arc::clone(&queue);
            ehttp::fetch(ehttp::Request::get(&url), move |result| {
                let bytes = result.ok().filter(|response| response.ok).map(|r| r.bytes);
                let mut guard = progress.lock().unwrap();
                match bytes {
                    Some(bytes) => {
                        guard.1.insert(key.clone(), bytes);
                        guard.2 += 1;
                    }
                    None => guard.3 = true,
                }
                if guard.3 {
                    return;
                }
                if guard.2 == total {
                    let gltf = guard.0.take().unwrap();
                    let resources = std::mem::take(&mut guard.1);
                    if let Ok(mut models) = queue.lock() {
                        models.push(AgentModelLoad {
                            correlation_id,
                            gltf,
                            resources,
                        });
                    }
                }
            });
        }
    });
}

fn slots(viewer: &ViewerWorld) -> (AssetSlot, LoadingSlot) {
    (
        Arc::clone(&viewer.resources.incoming.asset),
        Arc::clone(&viewer.resources.incoming.loading),
    )
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

fn clear(loading: &LoadingSlot) {
    if let Ok(mut guard) = loading.lock() {
        *guard = None;
    }
}

fn download_single(
    url: String,
    make: fn(Vec<u8>) -> PendingAsset,
    asset: AssetSlot,
    loading: LoadingSlot,
) {
    ehttp::fetch(ehttp::Request::get(&url), move |result| {
        if let Ok(response) = result
            && response.ok
            && let Ok(mut slot) = asset.lock()
        {
            *slot = Some(make(response.bytes));
        }
        clear(&loading);
    });
}

fn download_model(
    gltf_url: String,
    includes: Vec<(String, String)>,
    asset: AssetSlot,
    loading: LoadingSlot,
) {
    ehttp::fetch(ehttp::Request::get(&gltf_url), move |result| {
        let gltf = match result.ok().filter(|response| response.ok).map(|r| r.bytes) {
            Some(bytes) => bytes,
            None => {
                clear(&loading);
                return;
            }
        };
        if includes.is_empty() {
            if let Ok(mut slot) = asset.lock() {
                *slot = Some(PendingAsset::ModelWithResources {
                    gltf,
                    resources: HashMap::new(),
                });
            }
            clear(&loading);
            return;
        }

        let total = includes.len();
        let progress = Arc::new(Mutex::new((Some(gltf), HashMap::new(), 0usize, false)));
        for (key, url) in includes {
            let progress = Arc::clone(&progress);
            let asset = Arc::clone(&asset);
            let loading = loading.clone();
            ehttp::fetch(ehttp::Request::get(&url), move |result| {
                let bytes = result.ok().filter(|response| response.ok).map(|r| r.bytes);
                let mut guard = progress.lock().unwrap();
                match bytes {
                    Some(bytes) => {
                        guard.1.insert(key.clone(), bytes);
                        guard.2 += 1;
                    }
                    None => guard.3 = true,
                }
                if guard.3 {
                    clear(&loading);
                    return;
                }
                if guard.2 == total {
                    let gltf = guard.0.take().unwrap();
                    let resources = std::mem::take(&mut guard.1);
                    if let Ok(mut slot) = asset.lock() {
                        *slot = Some(PendingAsset::ModelWithResources { gltf, resources });
                    }
                    clear(&loading);
                }
            });
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
            _ => FetchState::Failed,
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

fn ensure_polyhaven(state: &Arc<Mutex<FetchState<Vec<PolyAsset>>>>, index_url: &str) {
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
    ehttp::fetch(ehttp::Request::get(index_url), move |result| {
        let next = match result {
            Ok(response) if response.ok => match serde_json::from_slice::<
                std::collections::BTreeMap<String, PolyRaw>,
            >(&response.bytes)
            {
                Ok(raw) => FetchState::Loaded(poly_entries(raw)),
                Err(_) => FetchState::Failed,
            },
            _ => FetchState::Failed,
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

fn pick_hdr(files: HdriFiles, preferred: u32) -> Option<String> {
    let entries: Vec<(u32, String)> = files
        .hdri
        .into_iter()
        .filter_map(|(key, resolution)| {
            resolution
                .hdr
                .map(|link| (resolution_value(&key), link.url))
        })
        .collect();
    pick_resolution(entries, preferred)
}

fn pick_model(files: ModelFiles, preferred: u32) -> Option<GltfFile> {
    let entries: Vec<(u32, GltfFile)> = files
        .gltf
        .into_iter()
        .map(|(key, resolution)| (resolution_value(&key), resolution.gltf))
        .collect();
    pick_resolution(entries, preferred)
}

/// Picks the exact requested resolution, else the highest available below it,
/// else the smallest.
fn pick_resolution<T>(mut entries: Vec<(u32, T)>, preferred: u32) -> Option<T> {
    entries.sort_by_key(|(value, _)| *value);
    if let Some(index) = entries.iter().position(|(value, _)| *value == preferred) {
        return Some(entries.swap_remove(index).1);
    }
    if let Some(index) = entries.iter().rposition(|(value, _)| *value <= preferred) {
        return Some(entries.swap_remove(index).1);
    }
    entries.into_iter().next().map(|(_, value)| value)
}

fn resolution_value(key: &str) -> u32 {
    key.trim_end_matches(['k', 'K']).parse().unwrap_or(u32::MAX)
}
