use leptos::prelude::*;
use nightshade_viewer::App;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}
