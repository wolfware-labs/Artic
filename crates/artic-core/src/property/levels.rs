use crate::property::Precedence;
use crate::reactive::Change;

const OVERRIDES: usize = 5;

#[derive(Debug)]
pub struct Levels<T> {
  overrides: [Option<T>; OVERRIDES],
  default: T,
}

impl<T> Levels<T>
where
  T: PartialEq,
{
  pub fn new(default: T) -> Self {
    Self {
      overrides: [const { None }; OVERRIDES],
      default,
    }
  }

  pub fn effective(&self) -> &T {
    self.overrides.iter().rev().find_map(Option::as_ref).unwrap_or(&self.default)
  }

  pub fn effective_source(&self) -> Precedence {
    match self.overrides.iter().rposition(Option::is_some) {
      Some(index) => Self::level_at(index),
      None => Precedence::Default,
    }
  }

  pub fn set(&mut self, level: Precedence, value: T) -> Change {
    let winner = self.effective_source();
    match Self::override_index(level) {
      Some(index) => {
        if self.overrides[index].as_ref() == Some(&value) {
          return Change::Hidden;
        }
        self.overrides[index] = Some(value);
      }
      None => {
        if self.default == value {
          return Change::Hidden;
        }
        self.default = value;
      }
    }
    if level >= winner { Change::Visible } else { Change::Hidden }
  }

  pub fn clear(&mut self, level: Precedence) -> Change {
    let winner = self.effective_source();
    let index = Self::override_index(level).expect("the Default level can't be cleared");
    if self.overrides[index].take().is_some() && level == winner {
      Change::Visible
    } else {
      Change::Hidden
    }
  }

  fn override_index(level: Precedence) -> Option<usize> {
    (level as usize).checked_sub(1)
  }

  fn level_at(index: usize) -> Precedence {
    [
      Precedence::Inherited,
      Precedence::StyleSetter,
      Precedence::Binding,
      Precedence::Local,
      Precedence::Animation,
    ][index]
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;

  // Each override level maps to its own slot and back.
  #[test]
  fn override_index_round_trips() {
    for level in [
      Precedence::Inherited,
      Precedence::StyleSetter,
      Precedence::Binding,
      Precedence::Local,
      Precedence::Animation,
    ] {
      let index = Levels::<i32>::override_index(level).expect("an override level has a slot");
      assert_eq!(Levels::<i32>::level_at(index), level);
    }
    assert_eq!(Levels::<i32>::override_index(Precedence::Default), None);
  }

  // Writing the winning level or a level above it is visible; writing below it is hidden.
  #[test]
  fn set_is_visible_only_at_or_above_the_winner() {
    let mut levels = Levels::new(0);
    assert_eq!(levels.set(Precedence::Binding, 1), Change::Visible);

    assert_eq!(levels.set(Precedence::StyleSetter, 2), Change::Hidden);
    assert_eq!(levels.set(Precedence::Binding, 3), Change::Visible);
    assert_eq!(levels.set(Precedence::Local, 3), Change::Visible);
    assert_eq!(levels.effective(), &3);
    assert_eq!(levels.effective_source(), Precedence::Local);
  }

  // Writing the value a level already holds stores nothing and is hidden.
  #[test]
  fn set_with_the_same_value_is_hidden() {
    let mut levels = Levels::new(0);
    levels.set(Precedence::Local, 1);

    assert_eq!(levels.set(Precedence::Local, 1), Change::Hidden);
    assert_eq!(levels.set(Precedence::Default, 0), Change::Hidden);
  }

  // Writing the default is visible only while no override is set.
  #[test]
  fn set_default_is_visible_only_when_nothing_overrides_it() {
    let mut levels = Levels::new(0);
    assert_eq!(levels.set(Precedence::Default, 1), Change::Visible);

    levels.set(Precedence::Inherited, 2);

    assert_eq!(levels.set(Precedence::Default, 3), Change::Hidden);
    assert_eq!(levels.effective(), &2);
  }

  // Clearing the winning level is visible and reveals the next one; clearing any other level is
  // hidden.
  #[test]
  fn clear_is_visible_only_for_the_winner() {
    let mut levels = Levels::new(0);
    levels.set(Precedence::StyleSetter, 1);
    levels.set(Precedence::Local, 2);

    assert_eq!(levels.clear(Precedence::Binding), Change::Hidden);
    assert_eq!(levels.clear(Precedence::StyleSetter), Change::Hidden);
    assert_eq!(levels.clear(Precedence::Local), Change::Visible);
    assert_eq!(levels.effective(), &0);
    assert_eq!(levels.effective_source(), Precedence::Default);
  }
}
