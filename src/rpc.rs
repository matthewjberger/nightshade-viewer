use crate::api::{publish_event, Event, WebsocketEvent};
use crate::prelude::*;
use enum2egui::{Gui, GuiInspect};
use enum2str::EnumStr;

// Remote Procedure Calls
#[derive(Default)]
pub struct Rpc {
    pub sender: Option<ewebsock::WsSender>,
    pub receiver: Option<ewebsock::WsReceiver>,
    pub is_connected: bool,
}

#[derive(Default, Debug, Clone, Gui, EnumStr)]
pub enum RpcCommand {
    #[default]
    Empty,
    Connect {
        url: String,
    },
    Disconnect,
    Send {
        message: RpcMessage,
    },
}

#[derive(Default, Debug, Clone, Gui, EnumStr)]
pub enum RpcMessage {
    #[default]
    Empty,
    Text {
        string: String,
    },
    Binary {
        bytes: Vec<u8>,
    },
}

#[derive(Default, Debug, Clone, Gui, EnumStr)]
pub enum RpcEvent {
    #[default]
    Empty,
    ConnectionAttemptSucceeded,
    ConnectionAttemptStarted,
    Disconnected,
    Message {
        message: RpcMessage,
    },
    Error {
        error: RpcError,
    },
}

#[derive(Default, Debug, Clone, Gui, EnumStr)]
pub enum RpcError {
    #[default]
    Empty,
    ConnectionFailed {
        url: String,
    },
    Server {
        error: String,
    },
    SendFailed {
        message: RpcMessage,
    },
}

pub fn receive_rpc_events_system(context: &mut Context) {
    dequeue_rpc_events(context).into_iter().for_each(|event| {
        receive_rpc_event(context, event);
    });
}

fn dequeue_rpc_events(context: &mut Context) -> Vec<ewebsock::WsEvent> {
    let Some(receiver) = context.resources.rpc.receiver.as_mut() else {
        return Vec::new();
    };
    let mut events = Vec::new();
    while let Some(event) = receiver.try_recv() {
        events.push(event);
    }
    events
}

fn receive_rpc_event(context: &mut Context, event: ewebsock::WsEvent) {
    match event {
        ewebsock::WsEvent::Opened => {
            context.resources.rpc.is_connected = true;
            handle_websocket_connected(context);
        }
        ewebsock::WsEvent::Message(ws_message) => match ws_message {
            ewebsock::WsMessage::Text(text) => {
                handle_websocket_message(context, text);
            }
            ewebsock::WsMessage::Binary(_) => {
                handle_websocket_error(context, "Binary messages not supported".to_string());
            }
            _ => {}
        },
        ewebsock::WsEvent::Error(error) => {
            context.resources.rpc.is_connected = false;
            handle_websocket_error(context, error.to_string());
        }
        ewebsock::WsEvent::Closed => {
            context.resources.rpc.is_connected = false;
            handle_websocket_disconnected(context);
        }
    }
}

pub fn execute_rpc_command(context: &mut Context, command: RpcCommand) {
    match command {
        RpcCommand::Connect { url } => {
            if context.resources.rpc.is_connected {
                handle_websocket_error(context, "Already connected".to_string());
                return;
            }

            if let Ok((sender, receiver)) =
                ewebsock::connect(format!("ws://{url}"), ewebsock::Options::default())
            {
                context.resources.rpc.sender = Some(sender);
                context.resources.rpc.receiver = Some(receiver);
                context.resources.rpc.is_connected = true;
                handle_websocket_connected(context);
            } else {
                handle_websocket_error(context, format!("Failed to connect to {}", url));
            }
        }
        RpcCommand::Send { message } => {
            if !context.resources.rpc.is_connected {
                handle_websocket_error(context, "Not connected".to_string());
                return;
            }

            if let Some(sender) = &mut context.resources.rpc.sender {
                sender.send(ewebsock::WsMessage::Text(message.to_string()));
                handle_websocket_message(context, format!("Sent: {}", message));
            }
        }
        RpcCommand::Disconnect => {
            context.resources.rpc.is_connected = false;
            context.resources.rpc.sender.take();
            context.resources.rpc.receiver.take();
            handle_websocket_disconnected(context);
        }
        _ => {}
    }
}

fn handle_websocket_connected(context: &mut Context) {
    publish_event(
        context,
        Event::Websocket {
            event: WebsocketEvent::Connected,
        },
    );
}

fn handle_websocket_disconnected(context: &mut Context) {
    publish_event(
        context,
        Event::Websocket {
            event: WebsocketEvent::Disconnected,
        },
    );
}

fn handle_websocket_message(context: &mut Context, text: String) {
    publish_event(
        context,
        Event::Websocket {
            event: WebsocketEvent::Message { message: text },
        },
    );
}

fn handle_websocket_error(context: &mut Context, error: String) {
    publish_event(
        context,
        Event::Websocket {
            event: WebsocketEvent::Error { error },
        },
    );
}
