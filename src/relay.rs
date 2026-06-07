use std::cell::RefCell;
use std::rc::Rc;

use protocol::{AgentRequest, AgentResponse, ClientMessage};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{MessageEvent, WebSocket};

use crate::bridge::{Bridge, send};

/// Where the native MCP bridge listens. The page is the websocket client; the
/// bridge is the server. Agent traffic rides this onto the existing worker
/// postMessage path.
const RELAY_URL: &str = "ws://127.0.0.1:8787";
const RECONNECT_MS: i32 = 1000;

/// The current relay socket, shared between the connect logic and the worker's
/// onmessage handler so agent responses can be sent back to the bridge.
pub type RelaySocket = Rc<RefCell<Option<WebSocket>>>;

/// Opens the relay and keeps it open, reconnecting on drop. Incoming agent
/// requests are forwarded to the worker; the worker's agent responses are sent
/// back through [`send_response`].
pub fn start(bridge: Bridge, socket: RelaySocket) {
    connect_once(bridge, socket);
}

fn connect_once(bridge: Bridge, socket: RelaySocket) {
    let Ok(websocket) = WebSocket::new(RELAY_URL) else {
        schedule_reconnect(bridge, socket);
        return;
    };

    let message_bridge = bridge.clone();
    let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        if let Some(text) = event.data().as_string()
            && let Ok(request) = serde_json::from_str::<AgentRequest>(&text)
        {
            send(&message_bridge, &ClientMessage::Agent(request));
        }
    });
    websocket.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    let close_bridge = bridge.clone();
    let close_socket = socket.clone();
    let onclose = Closure::<dyn FnMut()>::new(move || {
        schedule_reconnect(close_bridge.clone(), close_socket.clone());
    });
    websocket.set_onclose(Some(onclose.as_ref().unchecked_ref()));
    onclose.forget();

    *socket.borrow_mut() = Some(websocket);
}

fn schedule_reconnect(bridge: Bridge, socket: RelaySocket) {
    *socket.borrow_mut() = None;
    let Some(window) = web_sys::window() else {
        return;
    };
    let callback = Closure::<dyn FnMut()>::new(move || {
        connect_once(bridge.clone(), socket.clone());
    });
    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
        callback.as_ref().unchecked_ref(),
        RECONNECT_MS,
    );
    callback.forget();
}

/// Sends one agent response back to the bridge if the relay is open.
pub fn send_response(socket: &RelaySocket, response: &AgentResponse) {
    if let Some(websocket) = socket.borrow().as_ref()
        && websocket.ready_state() == WebSocket::OPEN
        && let Ok(text) = serde_json::to_string(response)
    {
        let _ = websocket.send_with_str(&text);
    }
}
