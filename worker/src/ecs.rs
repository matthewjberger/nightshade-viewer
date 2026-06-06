mod components;
mod resources;

pub use components::*;
pub use resources::*;

use nightshade::prelude::freecs;

freecs::ecs! {
    ViewerWorld {
        marker: Marker => MARKER,
    }
    Tags {
    }
    Events {
    }
    Resources {
        camera_input: CameraInput,
        selection: Selection,
        model: Model,
        picking: Picking,
        scene_sync: SceneSync,
        incoming: Incoming,
        browsers: Browsers,
    }
}
