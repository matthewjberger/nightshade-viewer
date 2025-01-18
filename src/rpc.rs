use crate::api::push_event;
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
            push_event(
                context,
                Event::Rpc {
                    event: RpcEvent::ConnectionAttemptStarted,
                },
            );
        }
        ewebsock::WsEvent::Message(ws_message) => match ws_message {
            ewebsock::WsMessage::Text(text) => {
                push_event(
                    context,
                    Event::Rpc {
                        event: RpcEvent::Message {
                            message: RpcMessage::Text { string: text },
                        },
                    },
                );
            }
            ewebsock::WsMessage::Binary(bytes) => {
                push_event(
                    context,
                    Event::Rpc {
                        event: RpcEvent::Message {
                            message: RpcMessage::Binary { bytes },
                        },
                    },
                );
            }
            _ => {}
        },
        ewebsock::WsEvent::Error(error) => {
            context.resources.rpc.is_connected = false;
            push_event(
                context,
                Event::Rpc {
                    event: RpcEvent::Error {
                        error: RpcError::Server {
                            error: error.to_string(),
                        },
                    },
                },
            );
        }
        ewebsock::WsEvent::Closed => {
            context.resources.rpc.is_connected = false;
            push_event(
                context,
                Event::Rpc {
                    event: RpcEvent::Disconnected,
                },
            );
        }
    }
}

pub fn execute_rpc_command(context: &mut Context, command: RpcCommand) {
    match command {
        RpcCommand::Connect { url } => {
            connect(context, &url);
        }
        RpcCommand::Disconnect => {
            disconnect(context);
        }
        RpcCommand::Send { message } => {
            send(context, message);
        }
        RpcCommand::Empty => {}
    }
}

fn connect(context: &mut Context, url: &str) {
    if let Ok((sender, receiver)) =
        ewebsock::connect(format!("ws://{url}"), ewebsock::Options::default())
    {
        context.resources.rpc.sender = Some(sender);
        context.resources.rpc.receiver = Some(receiver);
        push_event(
            context,
            Event::Rpc {
                event: RpcEvent::ConnectionAttemptStarted,
            },
        );
    } else {
        log::error!("Failed to connect to websocket server");
        push_event(
            context,
            Event::Rpc {
                event: RpcEvent::Error {
                    error: RpcError::ConnectionFailed {
                        url: url.to_string(),
                    },
                },
            },
        );
    }
}

fn disconnect(context: &mut Context) {
    context.resources.rpc.is_connected = false;
    context.resources.rpc.sender.take();
}

fn send(context: &mut Context, message: RpcMessage) {
    if let Some(sender) = context.resources.rpc.sender.as_mut() {
        match message {
            RpcMessage::Text { string: message } => {
                sender.send(ewebsock::WsMessage::Text(message));
            }
            RpcMessage::Binary { bytes } => {
                sender.send(ewebsock::WsMessage::Binary(bytes));
            }
            RpcMessage::Empty => {}
        }
    } else {
        log::error!("Attempted to send message but websocket is not connected");
        push_event(
            context,
            Event::Rpc {
                event: RpcEvent::Error {
                    error: RpcError::SendFailed { message },
                },
            },
        );
    }
}
