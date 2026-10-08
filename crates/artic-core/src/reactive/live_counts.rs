use crate::reactive::runtime::Runtime;

/// How many reactive nodes are alive on the current thread.
///
/// Leak checks take a snapshot before some work and compare it with one taken after disposing
/// that work's scope: every count should be back where it started. The counts are raw: `signals`
/// includes the signal that holds each memo's value.
///
/// # Examples
///
/// ```
/// use artic_core::reactive::{Signal, live_counts};
///
/// let before = live_counts();
/// Signal::new(0);
/// assert_eq!(live_counts().signals, before.signals + 1);
/// ```
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct LiveCounts {
  /// Live signals, including the ones that hold memo values.
  pub signals: usize,
  /// Live memos.
  pub memos: usize,
  /// Live effects.
  pub effects: usize,
  /// Live scopes.
  pub scopes: usize,
}

/// Returns how many reactive nodes are alive on the current thread.
///
/// See [`LiveCounts`].
pub fn live_counts() -> LiveCounts {
  Runtime::live_counts()
}
