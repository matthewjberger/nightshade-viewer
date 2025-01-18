use nightshade_engine::{
    self, log,
    nalgebra_glm::Vec3,
    prelude::*,
    serde_json,
    tokio_tungstenite::tungstenite::{connect, stream::MaybeTlsStream, Message, WebSocket},
};
use pyo3::{prelude::*, types::PyModule};
use url::Url;

// Internal type for handling responses
enum ResponseType {
    EntityCreated(u32),
    CameraList(Vec<u32>),
}

#[pyclass]
struct Client {
    ws: Option<WebSocket<MaybeTlsStream<std::net::TcpStream>>>,
    next_command_id: u64,
    pending_messages: Vec<(u64, ResponseType)>,
}

#[pymethods]
impl Client {
    #[new]
    fn new() -> Self {
        Self {
            ws: None,
            next_command_id: 0,
            pending_messages: Vec::new(),
        }
    }

    fn connect(&mut self, url: &str) -> PyResult<()> {
        let url = Url::parse(url).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Invalid URL: {}", e))
        })?;

        let (ws, _) = connect(url)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyConnectionError, _>(e.to_string()))?;

        self.ws = Some(ws);
        Ok(())
    }

    fn send_json(&mut self, json: &str) -> PyResult<()> {
        let ws = self
            .ws
            .as_mut()
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyConnectionError, _>("Not connected"))?;

        ws.send(Message::Text(json.to_string()))
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyConnectionError, _>(e.to_string()))?;

        Ok(())
    }

    fn wait_for_response(&mut self) -> PyResult<Option<String>> {
        let ws = self
            .ws
            .as_mut()
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyConnectionError, _>("Not connected"))?;

        if let Ok(msg) = ws.read() {
            if let Message::Text(text) = msg {
                log::debug!("[Python] Received websocket message: {}", text);
                return Ok(Some(text));
            }
        }
        Ok(None)
    }

    fn disconnect(&mut self) {
        if let Some(ws) = &mut self.ws {
            let _ = ws.close(None);
        }
        self.ws = None;
    }
}

// Move wait_for_command_response to a separate impl block (no #[pymethods])
impl Client {
    fn wait_for_command_response(&mut self, command_id: u64) -> PyResult<ResponseType> {
        log::debug!("[Python] Waiting for response to command {}", command_id);

        // Check pending messages first
        if let Some(pos) = self
            .pending_messages
            .iter()
            .position(|(id, _)| *id == command_id)
        {
            log::debug!("[Python] Found pending response for command {}", command_id);
            return Ok(self.pending_messages.remove(pos).1);
        }

        // Keep reading until we find our response
        while let Some(response) = self.wait_for_response()? {
            log::debug!("[Python] Received message: {}", response);
            if let Ok(event) = serde_json::from_str::<Event>(&response) {
                log::debug!("[Python] Parsed event: {:?}", event);
                let (source, response_type) = match event {
                    Event::EntityCreated { entity_id, source } => {
                        log::debug!(
                            "[Python] Got entity created event with ID {} and source {:?}",
                            entity_id.id,
                            source
                        );
                        (source, ResponseType::EntityCreated(entity_id.id))
                    }
                    Event::Report {
                        report: Report::Cameras { entity_ids },
                        source,
                    } => {
                        log::debug!("[Python] Got camera list with source {:?}", source);
                        (
                            source,
                            ResponseType::CameraList(
                                entity_ids.into_iter().map(|id| id.id).collect(),
                            ),
                        )
                    }
                    _ => continue,
                };

                if source == Some(command_id) {
                    log::debug!(
                        "[Python] Found matching response for command {}",
                        command_id
                    );
                    return Ok(response_type);
                }

                if let Some(src) = source {
                    log::debug!("[Python] Buffering response for command {}", src);
                    self.pending_messages.push((src, response_type));
                }
            }
        }

        Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
            "Connection closed while waiting for response",
        ))
    }
}

// Helper function to send a command and get its ID
fn send_command(client: &mut Client, command: Command) -> PyResult<u64> {
    let command_id = client.next_command_id;
    client.next_command_id += 1;

    // Send just the command, not wrapped in CommandMessage
    let json = serde_json::to_string(&command)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    client.send_json(&json)?;

    Ok(command_id)
}

#[pyfunction]
fn spawn_camera(client: &mut Client, x: f32, y: f32, z: f32, name: &str) -> PyResult<u64> {
    let command = Command::Entity {
        command: EntityCommand::SpawnCamera {
            position: Vec3::new(x, y, z),
            name: name.to_string(),
        },
    };

    let command_id = send_command(client, command)?;
    log::debug!("[Python] Sent spawn camera command with ID {}", command_id);

    match client.wait_for_command_response(command_id)? {
        ResponseType::EntityCreated(id) => {
            log::debug!(
                "[Python] Got camera entity ID {} for command {}",
                id,
                command_id
            );
            Ok(id.into())
        }
        _ => Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
            "Unexpected response type",
        )),
    }
}

#[pyfunction]
fn spawn_cube(client: &mut Client, x: f32, y: f32, z: f32, size: f32, name: &str) -> PyResult<u64> {
    let command = Command::Entity {
        command: EntityCommand::SpawnCube {
            position: Vec3::new(x, y, z),
            size,
            name: name.to_string(),
        },
    };

    let command_id = send_command(client, command)?;
    log::debug!("[Python] Sent spawn cube command with ID {}", command_id);

    match client.wait_for_command_response(command_id)? {
        ResponseType::EntityCreated(id) => {
            log::debug!(
                "[Python] Got cube entity ID {} for command {}",
                id,
                command_id
            );
            Ok(id.into())
        }
        _ => Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
            "Unexpected response type",
        )),
    }
}

#[pyfunction]
fn request_cameras(client: &mut Client) -> PyResult<Vec<u64>> {
    let command = Command::Request {
        command: RequestCommand::RequestCameraEntities,
    };

    let command_id = send_command(client, command)?;
    log::debug!(
        "[Python] Sent request cameras command with ID {}",
        command_id
    );

    match client.wait_for_command_response(command_id)? {
        ResponseType::CameraList(ids) => {
            log::debug!(
                "[Python] Got camera list with {} cameras for command {}",
                ids.len(),
                command_id
            );
            Ok(ids.into_iter().map(|id| id.into()).collect())
        }
        _ => Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
            "Unexpected response type",
        )),
    }
}

#[pymodule]
fn nightshade(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.setattr("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_class::<Client>()?;
    m.add_function(wrap_pyfunction!(spawn_camera, m)?)?;
    m.add_function(wrap_pyfunction!(spawn_cube, m)?)?;
    m.add_function(wrap_pyfunction!(request_cameras, m)?)?;
    Ok(())
}
