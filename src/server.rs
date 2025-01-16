use futures_util::{SinkExt, StreamExt, TryStreamExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Debug, Clone)]
pub enum ServerCommand {
    Connect { url: String },
}

pub async fn listen(port: u16) {
    let address = format!("0.0.0.0:{port}");

    let Ok(listener) = TcpListener::bind(&address).await else {
        log::error!("[Server] Failed to bind");
        return;
    };

    log::info!("[Server] Listening on: {address}");

    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(accept_connection(stream));
    }
}

async fn accept_connection(stream: TcpStream) {
    let Ok(address) = stream.peer_addr() else {
        log::warn!("[Server] Connected streams should have a peer address");
        return;
    };

    log::info!("[Server] Accepting connection from peer address: {address}");

    let Ok(websocket_stream) = tokio_tungstenite::accept_async(stream).await else {
        log::error!("[Server] Error during the websocket handshake occurred");
        return;
    };

    log::info!("[Server] Opened new WebSocket connection: {address}");

    let (mut write, mut read) = websocket_stream.split();

    while let Ok(Some(message)) = read.try_next().await {
        log::trace!("[Server] Received message: {message:?}");

        log::info!("[Server] Echoing message back to client...");
        if let Err(error) = write.send(message).await {
            log::error!("[Server] Failed to send response: {error}");
            break;
        }
    }

    log::info!("[Server] Connection closed: {address}");
}
