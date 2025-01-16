use crate::prelude::*;

#[derive(Default)]
pub struct Network {
    pub sender: Option<ewebsock::WsSender>,
    pub receiver: Option<ewebsock::WsReceiver>,
    pub commands: Vec<NetworkCommand>,
    pub is_connected: bool,
}

#[derive(Debug, Clone)]
pub enum NetworkCommand {
    Connect { url: String },
    Disconnect,
    Send { message: NetworkMessage },
}

#[derive(Debug, Clone)]
pub enum NetworkMessage {
    Text { string: String },
    Binary { bytes: Vec<u8> },
}

#[derive(Debug, Clone)]
pub enum NetworkEvent {
    ConnectionAttemptSucceeded,
    ConnectionAttemptStarted,
    Disconnected,
    Message { message: NetworkMessage },
    Error { error: NetworkError },
}

#[derive(Debug, Clone)]
pub enum NetworkError {
    ConnectionFailed { url: String },
    Server { error: String },
    SendFailed { message: NetworkMessage },
}

pub fn receive_network_events_system(context: &mut Context) {
    dequeue_websocket_events(context)
        .into_iter()
        .for_each(|event| {
            receive_websocket_events(context, event);
        });
}

fn dequeue_websocket_events(context: &mut Context) -> Vec<ewebsock::WsEvent> {
    let Some(receiver) = context.resources.network.receiver.as_mut() else {
        return Vec::new();
    };
    let mut events = Vec::new();
    while let Some(event) = receiver.try_recv() {
        events.push(event);
    }
    events
}

fn receive_websocket_events(context: &mut Context, event: ewebsock::WsEvent) {
    match event {
        ewebsock::WsEvent::Opened => {
            context.resources.network.is_connected = true;
            push_event(
                context,
                Event::Network {
                    event: NetworkEvent::ConnectionAttemptStarted,
                },
            );
        }
        ewebsock::WsEvent::Message(ws_message) => match ws_message {
            ewebsock::WsMessage::Text(text) => {
                push_event(
                    context,
                    Event::Network {
                        event: NetworkEvent::Message {
                            message: NetworkMessage::Text { string: text },
                        },
                    },
                );
            }
            ewebsock::WsMessage::Binary(bytes) => {
                push_event(
                    context,
                    Event::Network {
                        event: NetworkEvent::Message {
                            message: NetworkMessage::Binary { bytes },
                        },
                    },
                );
            }
            _ => {}
        },
        ewebsock::WsEvent::Error(error) => {
            context.resources.network.is_connected = false;
            push_event(
                context,
                Event::Network {
                    event: NetworkEvent::Error {
                        error: NetworkError::Server {
                            error: error.to_string(),
                        },
                    },
                },
            );
        }
        ewebsock::WsEvent::Closed => {
            context.resources.network.is_connected = false;
            push_event(
                context,
                Event::Network {
                    event: NetworkEvent::Disconnected,
                },
            );
        }
    }
}

pub fn execute_network_command(context: &mut Context, command: NetworkCommand) {
    match command {
        NetworkCommand::Connect { url } => {
            connect(context, &url);
        }
        NetworkCommand::Disconnect => {
            disconnect(context);
        }
        NetworkCommand::Send { message } => {
            send(context, message);
        }
    }
}

fn connect(context: &mut Context, url: &str) {
    if let Ok((sender, receiver)) =
        ewebsock::connect(format!("ws://{url}"), ewebsock::Options::default())
    {
        context.resources.network.sender = Some(sender);
        context.resources.network.receiver = Some(receiver);
        push_event(
            context,
            Event::Network {
                event: NetworkEvent::ConnectionAttemptStarted,
            },
        );
    } else {
        log::error!("Failed to connect to websocket server");
        push_event(
            context,
            Event::Network {
                event: NetworkEvent::Error {
                    error: NetworkError::ConnectionFailed {
                        url: url.to_string(),
                    },
                },
            },
        );
    }
}

fn disconnect(context: &mut Context) {
    context.resources.network.is_connected = false;
    context.resources.network.sender.take();
}

fn send(context: &mut Context, message: NetworkMessage) {
    if let Some(sender) = context.resources.network.sender.as_mut() {
        match message {
            NetworkMessage::Text { string: message } => {
                sender.send(ewebsock::WsMessage::Text(message));
            }
            NetworkMessage::Binary { bytes } => {
                sender.send(ewebsock::WsMessage::Binary(bytes));
            }
        }
    } else {
        log::error!("Attempted to send message but websocket is not connected");
        push_event(
            context,
            Event::Network {
                event: NetworkEvent::Error {
                    error: NetworkError::SendFailed { message },
                },
            },
        );
    }
}
