//! AHB — AI HP Bar — crate root.
//!
//! Phase 0 lint floor inherited via per-file `#![deny(...)]` attributes (see `main.rs`).
//! `lib.rs` carries the same floor so library consumers + integration tests are gated
//! through the same checks.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

pub mod model;
