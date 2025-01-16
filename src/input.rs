/// Contains all input state
#[derive(Default)]
pub struct Input {
    pub keyboard: Keyboard,
    pub mouse: Mouse,
}

/// Contains keyboard-specific input state
#[derive(Default)]
pub struct Keyboard {
    pub keystates: std::collections::HashMap<winit::keyboard::KeyCode, winit::event::ElementState>,
}

impl Keyboard {
    pub fn is_key_pressed(&self, keycode: winit::keyboard::KeyCode) -> bool {
        self.keystates
            .get(&keycode)
            .is_some_and(|state| *state == winit::event::ElementState::Pressed)
    }
}

bitflags::bitflags! {
    #[derive(Default, Debug, Clone, Copy)]
    pub struct MouseState: u8 {
        const LEFT_CLICKED = 0b0000_0001;
        const MIDDLE_CLICKED = 0b0000_0010;
        const RIGHT_CLICKED = 0b0000_0100;
        const MOVED = 0b0000_1000;
        const SCROLLED = 0b0001_0000;
    }
}

/// Contains mouse-specific input state
#[derive(Default, Debug, Clone, Copy)]
pub struct Mouse {
    pub state: MouseState,
    pub position: nalgebra_glm::Vec2,
    pub position_delta: nalgebra_glm::Vec2,
    pub wheel_delta: nalgebra_glm::Vec2,
}

pub fn escape_key_exit_system(context: &mut crate::context::Context) {
    let keyboard = &context.resources.input.keyboard;
    if keyboard.is_key_pressed(winit::keyboard::KeyCode::Escape) {
        context.resources.window.should_exit = true;
    }
}

/// Resets the input state for the next frame
pub fn reset_input_system(context: &mut crate::context::Context) {
    let mouse = &mut context.resources.input.mouse;
    if mouse.state.contains(crate::input::MouseState::SCROLLED) {
        mouse.wheel_delta = nalgebra_glm::vec2(0.0, 0.0);
    }
    mouse.state.set(crate::input::MouseState::MOVED, false);
    if !mouse.state.contains(crate::input::MouseState::MOVED) {
        mouse.position_delta = nalgebra_glm::vec2(0.0, 0.0);
    }
    mouse.state.set(crate::input::MouseState::MOVED, false);
}

pub fn receive_input_event(
    context: &mut crate::context::Context,
    event: &winit::event::WindowEvent,
) {
    receive_winit_keyboard_events(context, event);
    receive_winit_mouse_events(context, event);
}

fn receive_winit_keyboard_events(context: &mut crate::Context, event: &winit::event::WindowEvent) {
    let winit::event::WindowEvent::KeyboardInput {
        event:
            winit::event::KeyEvent {
                physical_key: winit::keyboard::PhysicalKey::Code(key_code),
                state,
                ..
            },
        ..
    } = event
    else {
        return;
    };
    *context
        .resources
        .input
        .keyboard
        .keystates
        .entry(*key_code)
        .or_insert(*state) = *state;
}

fn receive_winit_mouse_events(context: &mut crate::Context, event: &winit::event::WindowEvent) {
    let mouse = &mut context.resources.input.mouse;
    match event {
        winit::event::WindowEvent::MouseInput { button, state, .. } => {
            let clicked = *state == winit::event::ElementState::Pressed;
            match button {
                winit::event::MouseButton::Left => {
                    mouse
                        .state
                        .set(crate::input::MouseState::LEFT_CLICKED, clicked);
                }
                winit::event::MouseButton::Middle => {
                    mouse
                        .state
                        .set(crate::input::MouseState::MIDDLE_CLICKED, clicked);
                }
                winit::event::MouseButton::Right => {
                    mouse
                        .state
                        .set(crate::input::MouseState::RIGHT_CLICKED, clicked);
                }
                _ => {}
            }
        }
        winit::event::WindowEvent::CursorMoved { position, .. } => {
            let last_position = mouse.position;
            let current_position = nalgebra_glm::vec2(position.x as _, position.y as _);
            mouse.position = current_position;
            mouse.position_delta = current_position - last_position;
            mouse.state.set(crate::input::MouseState::MOVED, true);
        }
        winit::event::WindowEvent::MouseWheel {
            delta: winit::event::MouseScrollDelta::LineDelta(h_lines, v_lines),
            ..
        } => {
            mouse.wheel_delta = nalgebra_glm::vec2(*h_lines, *v_lines);
            mouse.state.set(crate::input::MouseState::SCROLLED, true);
        }
        _ => {}
    }
}
