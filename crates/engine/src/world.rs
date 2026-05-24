//! `World` is the owning container for the simulation state.
//!
//! Step 1 stub. Real implementation in Step 5.

#[derive(Default)]
pub struct World {
    pub tick: u64,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }
}
