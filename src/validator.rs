use leptos::prelude::Set;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::state::{Validation, ViewerState};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = globalThis, js_name = __validateGltf, catch)]
    async fn validate_gltf(
        bytes: js_sys::Uint8Array,
        resources: JsValue,
    ) -> Result<JsValue, JsValue>;
}

/// Runs the Khronos glTF-Validator (a JS module loaded by `runtime/validator.js`)
/// over a dropped model and stores the error and warning counts. Best-effort:
/// any failure clears the result.
pub fn validate(state: ViewerState, bytes: Vec<u8>, resources: Vec<(String, Vec<u8>)>) {
    spawn_local(async move {
        let array = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
        array.copy_from(&bytes);

        let resource_object = if resources.is_empty() {
            JsValue::NULL
        } else {
            let object = js_sys::Object::new();
            for (name, data) in resources {
                let value = js_sys::Uint8Array::new_with_length(data.len() as u32);
                value.copy_from(&data);
                let _ = js_sys::Reflect::set(&object, &JsValue::from_str(&name), &value);
            }
            object.into()
        };

        match validate_gltf(array, resource_object).await {
            Ok(report) => {
                let issues = js_sys::Reflect::get(&report, &JsValue::from_str("issues"))
                    .unwrap_or(JsValue::NULL);
                state.validation.set(Some(Validation {
                    errors: count(&issues, "numErrors"),
                    warnings: count(&issues, "numWarnings"),
                }));
            }
            Err(_) => state.validation.set(None),
        }
    });
}

fn count(issues: &JsValue, key: &str) -> u32 {
    js_sys::Reflect::get(issues, &JsValue::from_str(key))
        .ok()
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0) as u32
}
