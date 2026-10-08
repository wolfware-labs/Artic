//! Properties: reactive values that several sources can write at once, ranked by precedence.

/// Who supplied a property's value, from lowest to highest priority.
///
/// The ordering follows priority, so a higher level compares greater:
/// `Animation` > `Local` > `Binding` > `StyleSetter` > `Inherited` > `Default`.
///
/// # Examples
///
/// ```
/// use artic_core::property::Precedence;
///
/// assert!(Precedence::Animation > Precedence::Local);
/// assert!(Precedence::StyleSetter > Precedence::Default);
/// ```
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Precedence {
  /// The value a property has when no other level is set.
  Default,
  /// A value inherited from an ancestor node.
  Inherited,
  /// A value set by a matching style.
  StyleSetter,
  /// A value written by a data binding.
  Binding,
  /// A value set directly: in markup, or with `set` in code.
  Local,
  /// A value written by a running animation.
  Animation,
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;

  // Sorting the levels puts them in priority order, lowest first.
  #[test]
  fn precedence_orders_levels_by_priority() {
    let mut levels = [
      Precedence::Local,
      Precedence::Default,
      Precedence::Animation,
      Precedence::StyleSetter,
      Precedence::Inherited,
      Precedence::Binding,
    ];

    levels.sort();

    assert_eq!(
      levels,
      [
        Precedence::Default,
        Precedence::Inherited,
        Precedence::StyleSetter,
        Precedence::Binding,
        Precedence::Local,
        Precedence::Animation,
      ]
    );
  }
}
