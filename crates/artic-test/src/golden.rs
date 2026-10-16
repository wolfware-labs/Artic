//! Golden-image tests: draw a scene into an [`Image`] and check it against a PNG checked into the
//! repository.

mod image;

pub use image::Image;

#[cfg(test)]
pub(crate) fn scratch(name: &str) -> std::path::PathBuf {
  let dir = std::env::temp_dir().join(format!("artic-golden-{name}-{}", std::process::id()));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(&dir).expect("a scratch folder");
  dir
}
