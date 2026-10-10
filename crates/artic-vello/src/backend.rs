use vello::wgpu::Backends;

/// The GPU API to use on `os`, as [`std::env::consts::OS`] names it: DX12 on Windows, Metal on
/// macOS and iOS, Vulkan elsewhere.
pub(crate) fn backends_for(os: &str) -> Backends {
  match os {
    "windows" => Backends::DX12,
    "macos" | "ios" => Backends::METAL,
    _ => Backends::VULKAN,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;

  // Each desktop OS gets its native GPU API, whichever OS runs the test.
  #[test]
  fn backend_for_target() {
    let chosen: Vec<Backends> = ["windows", "macos", "linux"].into_iter().map(backends_for).collect();

    assert_eq!(chosen, [Backends::DX12, Backends::METAL, Backends::VULKAN]);
  }
}
