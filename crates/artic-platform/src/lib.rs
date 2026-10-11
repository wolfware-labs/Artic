//! The host seam: how artic talks to whatever drives the screen.
//!
//! - [`Host`] owns the event loop and runs an [`AppHandler`], artic's side of it.
//! - [`HostContext`] is handed to every [`AppHandler`] call: frames, exiting, and the capabilities,
//!   such as [`Windows`], each `None` on hosts without it.
//! - [`SurfaceInfo`], [`DisplayId`] and [`HostEvent`] are what a host reports, and [`Waker`] wakes
//!   its loop from other threads.
//!
//! This crate has no backend dependencies; adapter crates such as `artic-winit` implement it. The
//! design is `docs/design/host.md`.

mod event;
mod host;
mod surface;
mod waker;

pub use event::HostEvent;
pub use host::{AppHandler, Host, HostContext, Windows};
pub use surface::{DisplayId, SurfaceInfo};
pub use waker::Waker;
