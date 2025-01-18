use crate::prelude::*;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};

type CommandSender = Mutex<mpsc::UnboundedSender<Command>>;
type CommandReceiver = Mutex<mpsc::UnboundedReceiver<Command>>;
type EventSender = Mutex<mpsc::UnboundedSender<Event>>;
type EventReceiver = Mutex<mpsc::UnboundedReceiver<Event>>;

static COMMAND_CHANNEL: Lazy<(CommandSender, CommandReceiver)> = Lazy::new(|| {
    let (sender, receiver) = mpsc::unbounded_channel();
    (Mutex::new(sender), Mutex::new(receiver))
});

pub static EVENT_CHANNEL: Lazy<(EventSender, EventReceiver)> = Lazy::new(|| {
    let (sender, receiver) = mpsc::unbounded_channel();
    (Mutex::new(sender), Mutex::new(receiver))
});

// Inter-Process Communication
#[derive(Default)]
pub struct Ipc {
    pub sender: Option<ewebsock::WsSender>,
    pub receiver: Option<ewebsock::WsReceiver>,
    pub is_connected: bool,
    pub command_receiver: Option<mpsc::UnboundedReceiver<Command>>,
    pub next_command_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcCommand {
    Connect { url: String },
    Disconnect,
    Send { message: IpcMessage },
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcMessage {
    Text { string: String },
    Binary { bytes: Vec<u8> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcCommandMessage {
    pub id: u64,
    pub command: IpcCommand,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcEvent {
    ConnectionAttemptSucceeded,
    ConnectionAttemptStarted,
    Disconnected,
    Message { message: IpcMessage },
    Error { error: IpcError },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcError {
    ConnectionFailed { url: String },
    Server { error: String },
    SendFailed { message: IpcMessage },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub id: u64,
    pub result: IpcResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcResult {
    Success,
    Error { message: String },
}

pub fn execute_ipc_command(context: &mut Context, command: IpcCommand) {
    let command_message = IpcCommandMessage {
        id: context.resources.ipc.next_command_id,
        command: command.clone(),
    };
    let command_id = command_message.id;
    log::debug!("Executing IPC command {} - {:?}", command_id, command);
    context.resources.ipc.next_command_id = context.resources.ipc.next_command_id.wrapping_add(1);

    match command {
        IpcCommand::Connect { url } => {
            connect(context, &url, command_id);
        }
        IpcCommand::Disconnect => {
            disconnect(context);
        }
        IpcCommand::Send { message } => {
            send(context, message, command_id);
        }
    }
}

fn connect(context: &mut Context, url: &str, command_id: u64) {
    if let Ok((sender, receiver)) =
        ewebsock::connect(format!("ws://{url}"), ewebsock::Options::default())
    {
        context.resources.ipc.sender = Some(sender);
        context.resources.ipc.receiver = Some(receiver);
        push_event(
            context,
            Event::Ipc {
                event: IpcEvent::ConnectionAttemptStarted,
                source: Some(command_id),
            },
        );
    } else {
        log::error!("Failed to connect to IPC websocket server");
        push_event(
            context,
            Event::Ipc {
                event: IpcEvent::Error {
                    error: IpcError::ConnectionFailed {
                        url: url.to_string(),
                    },
                },
                source: Some(command_id),
            },
        );
    }
}

fn disconnect(context: &mut Context) {
    context.resources.ipc.is_connected = false;
    context.resources.ipc.sender.take();
}

fn send(context: &mut Context, message: IpcMessage, command_id: u64) {
    if let Some(sender) = context.resources.ipc.sender.as_mut() {
        let command_message = IpcCommandMessage {
            id: command_id,
            command: IpcCommand::Send {
                message: message.clone(),
            },
        };

        match serde_json::to_string(&command_message) {
            Ok(json) => {
                sender.send(ewebsock::WsMessage::Text(json));
            }
            Err(e) => {
                log::error!("Failed to serialize IPC command: {}", e);
                push_event(
                    context,
                    Event::Ipc {
                        event: IpcEvent::Error {
                            error: IpcError::SendFailed { message },
                        },
                        source: Some(command_id),
                    },
                );
            }
        }
    } else {
        log::error!("Attempted to send IPC message but websocket is not connected");
        push_event(
            context,
            Event::Ipc {
                event: IpcEvent::Error {
                    error: IpcError::SendFailed { message },
                },
                source: Some(command_id),
            },
        );
    }
}

fn receive_ipc_event(context: &mut Context, event: ewebsock::WsEvent) {
    match event {
        ewebsock::WsEvent::Opened => {
            context.resources.ipc.is_connected = true;
            push_event(
                context,
                Event::Ipc {
                    event: IpcEvent::ConnectionAttemptStarted,
                    source: None,
                },
            );
        }
        ewebsock::WsEvent::Message(ws_message) => match ws_message {
            ewebsock::WsMessage::Text(text) => {
                push_event(
                    context,
                    Event::Ipc {
                        event: IpcEvent::Message {
                            message: IpcMessage::Text { string: text },
                        },
                        source: None,
                    },
                );
            }
            ewebsock::WsMessage::Binary(bytes) => {
                push_event(
                    context,
                    Event::Ipc {
                        event: IpcEvent::Message {
                            message: IpcMessage::Binary { bytes },
                        },
                        source: None,
                    },
                );
            }
            _ => {}
        },
        ewebsock::WsEvent::Error(error) => {
            context.resources.ipc.is_connected = false;
            push_event(
                context,
                Event::Ipc {
                    event: IpcEvent::Error {
                        error: IpcError::Server {
                            error: error.to_string(),
                        },
                    },
                    source: None,
                },
            );
        }
        ewebsock::WsEvent::Closed => {
            context.resources.ipc.is_connected = false;
            push_event(
                context,
                Event::Ipc {
                    event: IpcEvent::Disconnected,
                    source: None,
                },
            );
        }
    }
}

// Add this helper struct for parsing
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum IncomingMessage {
    Command(Command),
    CommandMessage(CommandMessage),
}

// In push_event_with_source, also send to EVENT_CHANNEL
pub fn push_event_with_source(context: &mut Context, event: Event, source: u64) {
    log::debug!("[API] Pushing event with source {}: {:?}", source, event);
    let event_with_source = match event {
        Event::EntityCreated { entity_id, .. } => Event::EntityCreated {
            entity_id,
            source: Some(source),
        },
        Event::Report { report, .. } => Event::Report {
            report,
            source: Some(source),
        },
        Event::Rpc { event, .. } => Event::Rpc {
            event,
            source: Some(source),
        },
        Event::Ipc { event, .. } => Event::Ipc {
            event,
            source: Some(source),
        },
    };

    // First push to context
    push_event(context, event_with_source.clone());

    // Then send to IPC channel
    if let Ok(sender) = EVENT_CHANNEL.0.try_lock() {
        log::debug!("[IPC] Sending event to channel: {:?}", event_with_source);
        if let Err(e) = sender.send(event_with_source) {
            log::error!("[API] Failed to send event to IPC channel: {}", e);
        } else {
            log::debug!("[IPC] Successfully sent event to channel");
        }
    } else {
        log::error!("[IPC] Failed to lock event channel sender");
    }
}

#[cfg(not(target_arch = "wasm32"))]
// This backend listens for incoming IPC connections
pub async fn run_python_ipc_backend(port: u16) {
    use futures::SinkExt;
    use futures_util::{StreamExt, TryStreamExt};
    use tokio::net::TcpListener;
    use tokio::sync::mpsc;
    use tokio_tungstenite::tungstenite::Message;

    let address = format!("0.0.0.0:{port}");
    let mut next_command_id = 0u64;

    let Ok(listener) = TcpListener::bind(&address).await else {
        log::error!("[Python] Failed to bind");
        return;
    };

    log::debug!("[Python] Listening on: {address}");

    while let Ok((stream, _)) = listener.accept().await {
        // Create a new event receiver for each connection
        let event_receiver = EVENT_CHANNEL.1.try_lock().ok();

        tokio::spawn(async move {
            let Ok(address) = stream.peer_addr() else {
                log::warn!("[Python] Connected streams should have a peer address");
                return;
            };
            log::debug!("[Python] Accepting connection from peer address: {address}");
            let Ok(websocket_stream) = tokio_tungstenite::accept_async(stream).await else {
                log::error!("[Python] Error during the websocket handshake occurred");
                return;
            };
            log::debug!("[Python] Opened new WebSocket connection: {address}");

            // Create a channel for sending messages to the write task
            let (tx, mut rx) = mpsc::unbounded_channel();
            let (write, read) = websocket_stream.split();
            let mut write = write;

            // Spawn write task
            let write_task = tokio::spawn(async move {
                while let Some(msg) = rx.recv().await {
                    if let Err(e) = write.send(msg).await {
                        log::error!("[Python] Failed to send message: {}", e);
                        break;
                    }
                }
            });

            // Spawn event forwarding task
            if let Some(mut receiver) = event_receiver {
                let tx = tx.clone();
                tokio::spawn(async move {
                    log::debug!("[IPC] Starting event forwarding task");
                    while let Some(event) = receiver.recv().await {
                        log::debug!("[IPC] Forwarding event: {:?}", event);
                        if let Ok(json) = serde_json::to_string(&event) {
                            log::debug!("[IPC] Serialized event: {}", json);
                            match tx.send(Message::Text(json)) {
                                Ok(_) => log::debug!("[IPC] Successfully sent event to websocket"),
                                Err(e) => {
                                    log::error!("[IPC] Failed to forward event: {}", e);
                                    break;
                                }
                            }
                        } else {
                            log::error!("[IPC] Failed to serialize event");
                        }
                    }
                    log::debug!("[IPC] Event forwarding task ended");
                });
            } else {
                log::error!("[IPC] Failed to get event receiver");
            }

            // Handle incoming messages
            let mut read = read;
            while let Ok(Some(message)) = read.try_next().await {
                if let Message::Text(text) = message {
                    match serde_json::from_str::<IncomingMessage>(&text) {
                        Ok(message) => {
                            let command = match message {
                                IncomingMessage::Command(cmd) => cmd,
                                IncomingMessage::CommandMessage(msg) => msg.command,
                            };
                            log::debug!("[Python] Received command: {command:?}");
                            // Forward command through static channel
                            let mut success = true;
                            if let Ok(sender) = COMMAND_CHANNEL.0.try_lock() {
                                if let Err(e) = sender.send(command.clone()) {
                                    log::error!("[Python] Failed to forward command: {e}");
                                    success = false;
                                }
                            }

                            // Create response
                            let response = IpcResponse {
                                id: next_command_id,
                                result: if success {
                                    IpcResult::Success
                                } else {
                                    IpcResult::Error {
                                        message: "Failed to forward command".to_string(),
                                    }
                                },
                            };
                            next_command_id += 1;

                            // Send response using tx
                            if let Ok(json) = serde_json::to_string(&response) {
                                if let Err(e) = tx.send(Message::Text(json)) {
                                    log::error!("[Python] Failed to send response: {}", e);
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("[Python] Failed to parse command: {e}");
                            // Send error response without ID since we couldn't parse the command
                            let error_response = IpcResponse {
                                id: 0, // Use 0 for unparseable messages
                                result: IpcResult::Error {
                                    message: format!("Failed to parse command: {e}"),
                                },
                            };
                            if let Ok(json) = serde_json::to_string(&error_response) {
                                if let Err(e) = tx.send(Message::Text(json)) {
                                    log::error!("[Python] Failed to send error: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            write_task.abort(); // Clean up write task when done
            log::debug!("[Python] Connection closed: {address}");
        });
    }
}

// Add a system to process received commands
pub fn process_ipc_commands_system(context: &mut Context) {
    // Try to get commands from the static channel
    if let Ok(mut receiver) = COMMAND_CHANNEL.1.try_lock() {
        let mut commands = Vec::new();
        while let Ok(command) = receiver.try_recv() {
            commands.push(command);
        }
        for command in commands {
            push_command(context, command);
        }
    }
}

pub fn receive_ipc_events_system(context: &mut Context) {
    dequeue_ipc_events(context).into_iter().for_each(|event| {
        receive_ipc_event(context, event);
    });
}

fn dequeue_ipc_events(context: &mut Context) -> Vec<ewebsock::WsEvent> {
    let Some(receiver) = context.resources.ipc.receiver.as_mut() else {
        return Vec::new();
    };
    let mut events = Vec::new();
    while let Some(event) = receiver.try_recv() {
        events.push(event);
    }
    events
}
