//! Engine event channel types. Phase 1 declares the enum + buffer constant so Plan 03's
//! TUI subscribes via `mpsc::Receiver<EngineEvent>` without adding to `engine/` later.
//!
//! Channel-not-mutex (ARCHITECTURE.md Anti-Pattern 3): the TUI never shares state with
//! the engine via `Arc<Mutex<...>>`. Refresh results flow exclusively through this enum.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

use crate::model::{ProviderError, ProviderId, ProviderState};

/// Capacity of the engine→TUI mpsc channel. 64 is generous: three adapters × one tick
/// every 15 s = 0.2 messages/s, well below 64. CONTEXT discretion: `mpsc::channel(64)`.
pub const EVENT_BUFFER: usize = 64;

/// What the engine emits to subscribers. Plan 03's TUI is the first consumer.
/// Phase 1 emits `Refresh(...)` after every fetch tick; `TickError` is reserved for
/// orchestration-layer failures (NOT per-adapter failures, those live inside the Vec);
/// `Shutdown` lets the engine signal a clean teardown.
#[derive(Debug)]
pub enum EngineEvent {
    /// One batch of per-provider fetch results. Per-provider failures are encoded as
    /// `Err(ProviderError)` inside the Vec, so the channel never carries a tick-wide
    /// error caused by one slow adapter (ADP-01).
    Refresh(Vec<(ProviderId, Result<ProviderState, ProviderError>)>),
    /// Orchestration-level error (e.g., engine could not even build the fetch context).
    /// Rare; deferred handling is the subscriber's choice.
    TickError { source: anyhow::Error },
    /// Engine has stopped emitting. Subscribers should break their loop.
    Shutdown,
}
