//! Optional gamepad navigation, behind the `controller` cargo feature.
//!
//! Enabled with `--features controller`. On Linux this pulls `gilrs`, which
//! links `libudev` at build time; on Windows it uses XInput/RawInput with no
//! extra system dependency. Without the feature, [`Controller`] is a no-op so
//! the default build stays dependency-light and portable.

/// High-level navigation actions derived from gamepad input.
// Variants are only constructed when the `controller` feature is enabled.
#[cfg_attr(not(feature = "controller"), allow(dead_code))]
pub enum Action {
    /// Move selection up.
    Up,
    /// Move selection down.
    Down,
    /// Activate (launch) the current selection.
    Activate,
}

#[cfg(feature = "controller")]
pub use enabled::Controller;

#[cfg(not(feature = "controller"))]
pub use disabled::Controller;

#[cfg(feature = "controller")]
mod enabled {
    use super::Action;
    use gilrs::{Button, EventType, Gilrs};

    /// Wraps a `gilrs` context and translates button presses into [`Action`]s.
    pub struct Controller {
        gilrs: Option<Gilrs>,
    }

    impl Controller {
        pub fn new() -> Self {
            Self {
                gilrs: Gilrs::new().ok(),
            }
        }

        /// Whether at least one gamepad is connected.
        pub fn connected(&self) -> bool {
            self.gilrs
                .as_ref()
                .map(|g| g.gamepads().next().is_some())
                .unwrap_or(false)
        }

        /// Drain pending input and return the resulting actions.
        pub fn poll(&mut self) -> Vec<Action> {
            let mut actions = Vec::new();
            let Some(gilrs) = self.gilrs.as_mut() else {
                return actions;
            };
            while let Some(event) = gilrs.next_event() {
                if let EventType::ButtonPressed(button, _) = event.event {
                    match button {
                        Button::DPadUp => actions.push(Action::Up),
                        Button::DPadDown => actions.push(Action::Down),
                        Button::South => actions.push(Action::Activate),
                        _ => {}
                    }
                }
            }
            actions
        }
    }

    impl Default for Controller {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(not(feature = "controller"))]
mod disabled {
    use super::Action;

    /// No-op controller used when the `controller` feature is disabled.
    pub struct Controller;

    impl Controller {
        pub fn new() -> Self {
            Self
        }
        pub fn connected(&self) -> bool {
            false
        }
        pub fn poll(&mut self) -> Vec<Action> {
            Vec::new()
        }
    }

    impl Default for Controller {
        fn default() -> Self {
            Self::new()
        }
    }
}
