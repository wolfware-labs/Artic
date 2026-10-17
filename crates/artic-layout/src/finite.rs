//! Addition for positions and sizes that saturates instead of overflowing: two finite values
//! never add up to an infinity, while an infinite operand, such as an infinite available size,
//! stays infinite.

/// `a + b`, saturating at `f32::MAX` or `f32::MIN` by sign when finite operands overflow.
pub(crate) const fn add(a: f32, b: f32) -> f32 {
  saturated(a + b, a.is_finite() && b.is_finite())
}

const fn saturated(result: f32, finite_operands: bool) -> f32 {
  if finite_operands && result.is_infinite() {
    if result > 0.0 { f32::MAX } else { f32::MIN }
  } else {
    result
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  // Finite values that overflow saturate at the finite bound with the result's sign.
  #[test]
  fn finite_overflow_saturates_by_sign() {
    assert_eq!(add(f32::MAX, f32::MAX), f32::MAX);
    assert_eq!(add(f32::MIN, -f32::MAX), f32::MIN);
  }

  // An infinite operand isn't overflow: the result stays infinite.
  #[test]
  fn infinite_operands_stay_infinite() {
    assert_eq!(add(f32::INFINITY, -10.0), f32::INFINITY);
  }

  // Ordinary values add as usual.
  #[test]
  fn ordinary_values_are_unchanged() {
    assert_eq!(add(1.5, 2.0), 3.5);
  }
}
