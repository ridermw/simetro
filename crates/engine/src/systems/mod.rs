//! Simulation systems. Each tick runs a fixed pipeline:
//!
//! ```text
//!     tick_once(world) ──▶ lifecycle::run    (spawn Empty movers)
//!                       ──▶ movement::run    (advance Traveling movers)
//!                       ──▶ interaction::run (route arrived movers to next path)
//! ```
//!
//! Systems mutate the `World` and append `SimEvent`s (from
//! [`simetro_protocol`]) to a single per-tick output buffer owned
//! by the caller — no per-system allocations after warmup
//! (PLAN §14 zero-alloc target).

pub mod interaction;
pub mod lifecycle;
pub mod movement;
pub mod production;
