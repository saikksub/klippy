use crux_core::{
    Core,
    bridge::{Bridge, EffectId},
};

use crate::Klippy;

/// The interface the shell calls into. Events, effects and view models
/// cross the boundary as bincode-serialized bytes.
pub struct CoreFfi {
    core: Bridge<Klippy>,
}

impl Default for CoreFfi {
    fn default() -> Self {
        Self::new()
    }
}

#[boltffi::export]
impl CoreFfi {
    #[must_use]
    pub fn new() -> Self {
        Self {
            core: Bridge::new(Core::new()),
        }
    }

    /// Send an event to the app and return the requested effects.
    /// # Panics
    /// If the event cannot be deserialized.
    #[must_use]
    pub fn update(&self, data: &[u8]) -> Vec<u8> {
        let mut effects = Vec::new();
        match self.core.update(data, &mut effects) {
            Ok(()) => effects,
            Err(e) => panic!("{e}"),
        }
    }

    /// Resolve an effect with its output and return any further effects.
    /// # Panics
    /// If `data` cannot be deserialized or `id` is not a pending effect.
    #[must_use]
    pub fn resolve(&self, id: u32, data: &[u8]) -> Vec<u8> {
        let mut effects = Vec::new();
        match self.core.resolve(EffectId(id), data, &mut effects) {
            Ok(()) => effects,
            Err(e) => panic!("{e}"),
        }
    }

    /// Get the current serialized `ViewModel`.
    /// # Panics
    /// If the view model cannot be serialized.
    #[must_use]
    pub fn view(&self) -> Vec<u8> {
        let mut view_model = Vec::new();
        match self.core.view(&mut view_model) {
            Ok(()) => view_model,
            Err(e) => panic!("{e}"),
        }
    }
}
