/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

pub use platform::View;

#[derive(Debug, Copy, Clone)]
pub struct DrawableGeometry {
    pub view_size: (u32, u32),
    pub margins: (u32, u32, u32, u32),
    pub position: (i32, i32),
    pub hidpi_factor: f32,
}

/// View events

#[derive(Debug, Clone)]
pub enum ViewEvent {
    GeometryDidChange,
    MouseWheel(MouseScrollDelta, TouchPhase),
    MouseInput(ElementState, MouseButton),
    MouseMoved(i32, i32),
}

#[derive(Debug, Clone)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementState {
    Pressed,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone)]
pub enum MouseScrollDelta {
	LineDelta(f32, f32),
	PixelDelta(f32, f32)
}
