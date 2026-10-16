// Draws each scene of `artic_test::scenes` and checks it against its PNG in `tests/goldens`.
// `ARTIC_UPDATE_GOLDENS=1` rewrites the PNGs. Tests skip without a usable GPU, unless
// `ARTIC_REQUIRE_GPU=1`.

mod common;

use artic_render::Scene;
use artic_test::golden::render;
use artic_test::{golden, scenes};
use common::{is_warp, with_renderer};
use vello::wgpu::{Backend, DeviceType};

fn check(name: &str, scene: fn() -> Scene) {
  with_renderer(|renderer| {
    let image = render(renderer, &scene(), scenes::SIZE, 1.0).expect("renders");
    golden!(name).assert(&image);
  });
}

// WARP is a CPU adapter on DX12; lavapipe, a CPU adapter on Vulkan, and real GPUs are usable.
#[test]
fn only_dx12_cpu_adapters_are_warp() {
  assert!(is_warp(Backend::Dx12, DeviceType::Cpu));
  assert!(!is_warp(Backend::Vulkan, DeviceType::Cpu));
  assert!(!is_warp(Backend::Dx12, DeviceType::DiscreteGpu));
}

// A rounded rectangle with four different radii.
#[test]
fn rounded_rect_fill() {
  check("rounded_rect_fill", scenes::rounded_rect_fill);
}

// A star stroked with round joins.
#[test]
fn stroke() {
  check("stroke", scenes::stroke);
}

// A star filled even-odd.
#[test]
fn path() {
  check("path", scenes::path);
}

// A diagonal linear gradient of three stops.
#[test]
fn linear_gradient() {
  check("linear_gradient", scenes::linear_gradient);
}

// An elliptical radial gradient.
#[test]
fn radial_gradient() {
  check("radial_gradient", scenes::radial_gradient);
}

// A translucent card over its shadow.
#[test]
fn box_shadow() {
  check("box_shadow", scenes::box_shadow);
}

// A gradient clipped by a rectangle.
#[test]
fn rect_clip() {
  check("rect_clip", scenes::rect_clip);
}

// A gradient clipped by a circle.
#[test]
fn rounded_rect_clip() {
  check("rounded_rect_clip", scenes::rounded_rect_clip);
}

// A gradient clipped by a star.
#[test]
fn path_clip() {
  check("path_clip", scenes::path_clip);
}

// Two squares in nested opacity layers.
#[test]
fn nested_opacity() {
  check("nested_opacity", scenes::nested_opacity);
}
