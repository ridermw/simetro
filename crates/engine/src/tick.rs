//! Fixed-timestep tick loop.
//!
//! Step 1 stub. Real implementation in Steps 5/7.

use simetro_protocol::SimEvent;

#[derive(Debug, Default)]
pub struct TickOutput {
    pub events: Vec<SimEvent>,
    pub snapshot_dirty: bool,
}
