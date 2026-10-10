//! The vello renderer for artic: draws [`Scene`](artic_render::Scene)s with `vello` on `wgpu`.
//!
//! [`VelloRenderer`] is an adapter: it implements [`Renderer`](artic_render::Renderer) and nothing
//! else of artic depends on it. It draws on [`Target::Handles`](artic_render::Target::Handles)
//! (windows, through a swapchain per surface) and on [`Target::Pixels`](artic_render::Target::Pixels)
//! (offscreen, read back from the GPU), using DX12 on Windows, Metal on macOS and Vulkan elsewhere.
//! `WGPU_BACKEND` overrides the choice.
//!
//! Run `cargo run -p artic-vello --example window` to see it draw.

mod backend;
mod error;
mod renderer;
mod translate;

pub use error::VelloError;
pub use renderer::VelloRenderer;
