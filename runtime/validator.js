// Lazily loads the Khronos glTF-Validator and exposes a single async entry
// point for the wasm app. Best-effort: if the module fails to load, calls
// reject and the app simply shows no validation.

let validatorPromise;

function loadValidator() {
  if (!validatorPromise) {
    validatorPromise = import("https://cdn.jsdelivr.net/npm/gltf-validator/+esm")
      .then((module) => module.validateBytes || (module.default && module.default.validateBytes))
      .catch(() => null);
  }
  return validatorPromise;
}

globalThis.__validateGltf = async function (bytes, resources) {
  const validateBytes = await loadValidator();
  if (!validateBytes) {
    throw new Error("gltf validator unavailable");
  }
  const options = {};
  if (resources) {
    options.externalResourceFunction = function (uri) {
      const data = resources[uri] || resources[decodeURIComponent(uri)];
      return data
        ? Promise.resolve(data)
        : Promise.reject(new Error("missing resource: " + uri));
    };
  }
  return await validateBytes(bytes, options);
};
