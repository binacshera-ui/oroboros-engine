//! Input handling for UI.
//!
//! Processes mouse and keyboard input for widget interaction.

#![allow(missing_docs)]

/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    /// Left mouse button.
    Left,
    /// Right mouse button.
    Right,
    /// Middle mouse button (scroll wheel click).
    Middle,
}

/// Keyboard key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// Escape key.
    Escape,
    /// Enter/Return key.
    Enter,
    /// Tab key.
    Tab,
    /// Backspace key.
    Backspace,
    /// Delete key.
    Delete,
    /// Arrow up.
    Up,
    /// Arrow down.
    Down,
    /// Arrow left.
    Left,
    /// Arrow right.
    Right,
    /// Home key.
    Home,
    /// End key.
    End,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// Space bar.
    Space,
    /// Alphabetic keys.
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    /// More alphabetic keys.
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    /// Number keys.
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,
    /// Function keys.
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
}

/// Modifier keys state.
#[derive(Debug, Clone, Copy, Default)]
pub struct Modifiers {
    /// Shift key is held.
    pub shift: bool,
    /// Control key is held.
    pub ctrl: bool,
    /// Alt key is held.
    pub alt: bool,
    /// Super/Command key is held.
    pub super_key: bool,
}

/// Input state for the current frame.
#[derive(Debug, Clone, Default)]
pub struct InputState {
    /// Current mouse X position.
    pub mouse_x: f32,
    /// Current mouse Y position.
    pub mouse_y: f32,
    /// Mouse X position last frame.
    pub prev_mouse_x: f32,
    /// Mouse Y position last frame.
    pub prev_mouse_y: f32,
    /// Mouse buttons pressed this frame.
    buttons_pressed: u8,
    /// Mouse buttons released this frame.
    buttons_released: u8,
    /// Mouse buttons currently held.
    buttons_down: u8,
    /// Mouse scroll delta (x, y).
    pub scroll_delta: (f32, f32),
    /// Modifier keys state.
    pub modifiers: Modifiers,
    /// Keys pressed this frame.
    keys_pressed: Vec<Key>,
    /// Keys released this frame.
    keys_released: Vec<Key>,
    /// Keys currently held.
    keys_down: Vec<Key>,
    /// Text input this frame.
    pub text_input: String,
    /// Time since last click (for double-click detection).
    last_click_time: f32,
    /// Position of last click.
    last_click_pos: (f32, f32),
    /// Double-click detected this frame.
    double_clicked: bool,
}

impl InputState {
    /// Double-click time threshold (seconds).
    const DOUBLE_CLICK_TIME: f32 = 0.3;
    /// Double-click position threshold (pixels).
    const DOUBLE_CLICK_DISTANCE: f32 = 5.0;
    
    /// Creates a new empty input state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Begins a new frame, clearing per-frame state.
    pub fn begin_frame(&mut self) {
        self.prev_mouse_x = self.mouse_x;
        self.prev_mouse_y = self.mouse_y;
        self.buttons_pressed = 0;
        self.buttons_released = 0;
        self.scroll_delta = (0.0, 0.0);
        self.keys_pressed.clear();
        self.keys_released.clear();
        self.text_input.clear();
        self.double_clicked = false;
    }
    
    /// Updates mouse position.
    pub fn set_mouse_pos(&mut self, x: f32, y: f32) {
        self.mouse_x = x;
        self.mouse_y = y;
    }
    
    /// Records a mouse button press.
    pub fn mouse_button_down(&mut self, button: MouseButton, current_time: f32) {
        let mask = Self::button_mask(button);
        self.buttons_pressed |= mask;
        self.buttons_down |= mask;
        
        // Double-click detection
        if button == MouseButton::Left {
            let time_delta = current_time - self.last_click_time;
            let dx = self.mouse_x - self.last_click_pos.0;
            let dy = self.mouse_y - self.last_click_pos.1;
            let distance = (dx * dx + dy * dy).sqrt();
            
            if time_delta < Self::DOUBLE_CLICK_TIME && distance < Self::DOUBLE_CLICK_DISTANCE {
                self.double_clicked = true;
            }
            
            self.last_click_time = current_time;
            self.last_click_pos = (self.mouse_x, self.mouse_y);
        }
    }
    
    /// Records a mouse button release.
    pub fn mouse_button_up(&mut self, button: MouseButton) {
        let mask = Self::button_mask(button);
        self.buttons_released |= mask;
        self.buttons_down &= !mask;
    }
    
    /// Records scroll input.
    pub fn scroll(&mut self, dx: f32, dy: f32) {
        self.scroll_delta.0 += dx;
        self.scroll_delta.1 += dy;
    }
    
    /// Records a key press.
    pub fn key_down(&mut self, key: Key) {
        if !self.keys_down.contains(&key) {
            self.keys_pressed.push(key);
            self.keys_down.push(key);
        }
    }
    
    /// Records a key release.
    pub fn key_up(&mut self, key: Key) {
        self.keys_released.push(key);
        self.keys_down.retain(|&k| k != key);
    }
    
    /// Records text input.
    pub fn text(&mut self, text: &str) {
        self.text_input.push_str(text);
    }
    
    /// Returns true if the mouse button was clicked this frame.
    #[must_use]
    pub fn mouse_clicked(&self, button: MouseButton) -> bool {
        (self.buttons_pressed & Self::button_mask(button)) != 0
    }
    
    /// Returns true if the mouse button was released this frame.
    #[must_use]
    pub fn mouse_released(&self, button: MouseButton) -> bool {
        (self.buttons_released & Self::button_mask(button)) != 0
    }
    
    /// Returns true if the mouse button is currently held.
    #[must_use]
    pub fn mouse_down(&self, button: MouseButton) -> bool {
        (self.buttons_down & Self::button_mask(button)) != 0
    }
    
    /// Returns true if a double-click occurred this frame.
    #[must_use]
    pub fn double_clicked(&self) -> bool {
        self.double_clicked
    }
    
    /// Returns true if the key was pressed this frame.
    #[must_use]
    pub fn key_pressed(&self, key: Key) -> bool {
        self.keys_pressed.contains(&key)
    }
    
    /// Returns true if the key is currently held.
    #[must_use]
    pub fn key_held(&self, key: Key) -> bool {
        self.keys_down.contains(&key)
    }
    
    /// Returns the mouse movement delta.
    #[must_use]
    pub fn mouse_delta(&self) -> (f32, f32) {
        (self.mouse_x - self.prev_mouse_x, self.mouse_y - self.prev_mouse_y)
    }
    
    /// Returns the bit mask for a button.
    const fn button_mask(button: MouseButton) -> u8 {
        match button {
            MouseButton::Left => 1,
            MouseButton::Right => 2,
            MouseButton::Middle => 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mouse_click() {
        let mut input = InputState::new();
        
        input.mouse_button_down(MouseButton::Left, 0.0);
        assert!(input.mouse_clicked(MouseButton::Left));
        assert!(input.mouse_down(MouseButton::Left));
        
        input.begin_frame();
        assert!(!input.mouse_clicked(MouseButton::Left));
        assert!(input.mouse_down(MouseButton::Left));
        
        input.mouse_button_up(MouseButton::Left);
        assert!(input.mouse_released(MouseButton::Left));
        assert!(!input.mouse_down(MouseButton::Left));
    }
    
    #[test]
    fn test_double_click() {
        let mut input = InputState::new();
        
        input.mouse_button_down(MouseButton::Left, 0.0);
        input.begin_frame();
        input.mouse_button_down(MouseButton::Left, 0.1);
        
        assert!(input.double_clicked());
    }
}
