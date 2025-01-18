use crate::api::push_event_with_source;
use crate::prelude::*;
use serde::{Deserialize, Serialize};

// Remote Procedure Calls
#[derive(Default)]
pub struct Rpc {
    pub sender: Option<ewebsock::WsSender>,
    pub receiver: Option<ewebsock::WsReceiver>,
    pub is_connected: bool,
    pub next_command_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RpcCommand {
    Connect { url: String },
    Disconnect,
    Send { message: RpcMessage },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RpcMessage {
    Text { string: String },
    Binary { bytes: Vec<u8> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcCommandMessage {
    pub id: u64,
    pub command: RpcCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RpcEvent {
    ConnectionAttemptSucceeded,
    ConnectionAttemptStarted,
    Disconnected,
    Message { message: RpcMessage },
    Error { error: RpcError },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RpcError {
    ConnectionFailed { url: String },
    Server { error: String },
    SendFailed { message: RpcMessage },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub id: u64,
    pub result: RpcResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RpcResult {
    Success,
    Error { message: String },
}

pub fn receive_rpc_events_system(context: &mut Context) {
    let events = dequeue_rpc_events(context);
    if events.is_empty() {
        return;
    }
    log::debug!("[RPC] Dequeued {} events", events.len());
    events.into_iter().for_each(|event| {
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
    log::debug!("[RPC] Processing event: {:?}", event);
    match event {
        ewebsock::WsEvent::Opened => {
            context.resources.rpc.is_connected = true;
            push_event_with_source(
                context,
                Event::Rpc {
                    event: RpcEvent::ConnectionAttemptStarted,
                    source: None,
                },
                0,
            );
        }
        ewebsock::WsEvent::Message(ws_message) => match ws_message {
            ewebsock::WsMessage::Text(text) => {
                if let Ok(response) = serde_json::from_str::<RpcResponse>(&text) {
                    match response.result {
                        RpcResult::Success => {
                            log::debug!(
                                "[RPC] Skipping success response for command {}",
                                response.id
                            );
                        }
                        RpcResult::Error { .. } => {
                            push_event_with_source(
                                context,
                                Event::Rpc {
                                    event: RpcEvent::Message {
                                        message: RpcMessage::Text { string: text },
                                    },
                                    source: Some(response.id),
                                },
                                response.id,
                            );
                        }
                    }
                }
            }
            ewebsock::WsMessage::Binary(_bytes) => {
                // Binary messages are not responses, so ignore them
            }
            _ => {}
        },
        ewebsock::WsEvent::Error(error) => {
            context.resources.rpc.is_connected = false;
            push_event_with_source(
                context,
                Event::Rpc {
                    event: RpcEvent::Error {
                        error: RpcError::Server {
                            error: error.to_string(),
                        },
                    },
                    source: None,
                },
                0,
            );
        }
        ewebsock::WsEvent::Closed => {
            context.resources.rpc.is_connected = false;
            push_event_with_source(
                context,
                Event::Rpc {
                    event: RpcEvent::Disconnected,
                    source: None,
                },
                0,
            );
        }
    }
}

pub fn execute_rpc_command(context: &mut Context, command: RpcCommand) {
    let command_message = RpcCommandMessage {
        id: context.resources.rpc.next_command_id,
        command: command.clone(),
    };
    let command_id = command_message.id;
    log::debug!("Executing command {} - {:?}", command_id, command);
    context.resources.rpc.next_command_id = context.resources.rpc.next_command_id.wrapping_add(1);

    match command {
        RpcCommand::Connect { url } => {
            connect(context, &url, command_id);
        }
        RpcCommand::Disconnect => {
            disconnect(context);
        }
        RpcCommand::Send { message } => {
            send(context, message, command_id);
        }
    }
}

fn connect(context: &mut Context, url: &str, command_id: u64) {
    if let Ok((sender, receiver)) =
        ewebsock::connect(format!("ws://{url}"), ewebsock::Options::default())
    {
        context.resources.rpc.sender = Some(sender);
        context.resources.rpc.receiver = Some(receiver);
        push_event_with_source(
            context,
            Event::Rpc {
                event: RpcEvent::ConnectionAttemptStarted,
                source: Some(command_id),
            },
            command_id,
        );
    } else {
        log::error!("Failed to connect to websocket server");
        push_event_with_source(
            context,
            Event::Rpc {
                event: RpcEvent::Error {
                    error: RpcError::ConnectionFailed {
                        url: url.to_string(),
                    },
                },
                source: Some(command_id),
            },
            command_id,
        );
    }
}

fn disconnect(context: &mut Context) {
    context.resources.rpc.is_connected = false;
    context.resources.rpc.sender.take();
}

fn send(context: &mut Context, message: RpcMessage, command_id: u64) {
    if let Some(sender) = context.resources.rpc.sender.as_mut() {
        let command_message = RpcCommandMessage {
            id: command_id,
            command: RpcCommand::Send {
                message: message.clone(),
            },
        };

        match serde_json::to_string(&command_message) {
            Ok(json) => {
                sender.send(ewebsock::WsMessage::Text(json));
            }
            Err(e) => {
                log::error!("Failed to serialize RPC command: {}", e);
                push_event_with_source(
                    context,
                    Event::Rpc {
                        event: RpcEvent::Error {
                            error: RpcError::SendFailed { message },
                        },
                        source: Some(command_id),
                    },
                    command_id,
                );
            }
        }
    } else {
        log::error!("Attempted to send message but websocket is not connected");
        push_event_with_source(
            context,
            Event::Rpc {
                event: RpcEvent::Error {
                    error: RpcError::SendFailed { message },
                },
                source: Some(command_id),
            },
            command_id,
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn run_rpc_backend(port: u16) {
    use futures_util::{SinkExt, StreamExt, TryStreamExt};
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::Message;

    let address = format!("0.0.0.0:{port}");

    let Ok(listener) = TcpListener::bind(&address).await else {
        log::error!("[RPC] Failed to bind");
        return;
    };

    log::debug!("[RPC] Listening on: {address}");

    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(async move {
            let Ok(address) = stream.peer_addr() else {
                log::warn!("[RPC] Connected streams should have a peer address");
                return;
            };
            log::debug!("[RPC] Accepting connection from peer address: {address}");
            let Ok(websocket_stream) = tokio_tungstenite::accept_async(stream).await else {
                log::error!("[RPC] Error during the websocket handshake occurred");
                return;
            };
            log::debug!("[RPC] Opened new WebSocket connection: {address}");
            let (mut write, mut read) = websocket_stream.split();
            while let Ok(Some(message)) = read.try_next().await {
                if let Message::Text(text) = message {
                    if let Ok(command_message) = serde_json::from_str::<RpcCommandMessage>(&text) {
                        log::debug!(
                            "[RPC] Received command {}: {:?}",
                            command_message.id,
                            command_message.command
                        );

                        let response = RpcResponse {
                            id: command_message.id,
                            result: RpcResult::Success,
                        };
                        if let Ok(json) = serde_json::to_string(&response) {
                            if let Err(e) = write.send(Message::Text(json)).await {
                                log::error!("[RPC] Failed to send response: {}", e);
                                break;
                            }
                        }
                    }
                }
            }
            log::debug!("[RPC] Connection closed: {address}");
        });
    }
}
