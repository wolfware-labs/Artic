//! Golden-image tests: draw a scene into an [`Image`] and check it against a PNG checked into the
//! repository.

mod check;
mod image;

pub use check::{Golden, GoldenError, Tolerance};
pub use image::Image;

fn flag(name: &str) -> bool {
  std::env::var_os(name).is_some_and(|value| value == "1")
}

#[cfg(test)]
pub(crate) fn scratch(name: &str) -> std::path::PathBuf {
  let dir = std::env::temp_dir().join(format!("artic-golden-{name}-{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(&dir).expect("a scratch folder");
  dir
}
