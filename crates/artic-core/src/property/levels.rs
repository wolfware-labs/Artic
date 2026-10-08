use crate::property::{Affects, Precedence, Property, PropertyKey};
use crate::reactive::Change;
use std::cell::Cell;
use std::rc::Rc;

const VALUE_LEVELS: usize = 4;

#[derive(Debug)]
pub struct Levels<T>
where
  T: 'static,
{
  key: &'static PropertyKey<T>,
  dirty: Option<Rc<Cell<Affects>>>,
  values: [Option<T>; VALUE_LEVELS],
  inherited: Option<Property<T>>,
  default: T,
  followers: Vec<Property<T>>,
}

impl<T> Levels<T>
where
  T: PartialEq,
{
  pub fn new(key: &'static PropertyKey<T>, default: T) -> Self {
    Self {
      key,
      dirty: None,
      values: [const { None }; VALUE_LEVELS],
      inherited: None,
      default,
      followers: Vec::new(),
    }
  }

  pub fn key(&self) -> &'static PropertyKey<T> {
    self.key
  }

  pub fn attach(&mut self, dirty: Rc<Cell<Affects>>) {
    self.dirty = Some(dirty);
  }

  pub fn with_effective<TResult>(&self, function: impl FnOnce(&T) -> TResult) -> TResult {
    if let Some(value) = self.values.iter().rev().find_map(Option::as_ref) {
      return function(value);
    }
    match self.inherited {
      Some(source) => source.with(function),
      None => function(&self.default),
    }
  }

  pub fn effective_source(&self) -> Precedence {
    match self.values.iter().rposition(Option::is_some) {
      Some(index) => Self::level_at(index),
      None if self.inherited.is_some() => Precedence::Inherited,
      None => Precedence::Default,
    }
  }

  pub fn set(&mut self, level: Precedence, value: T) -> Change {
    let winner = self.effective_source();
    match Self::value_index(level) {
      Some(index) => {
        if self.values[index].as_ref() == Some(&value) {
          return Change::Hidden;
        }
        self.values[index] = Some(value);
      }
      None => {
        if self.default == value {
          return Change::Hidden;
        }
        self.default = value;
      }
    }
    if level >= winner {
      self.visible_change()
    } else {
      Change::Hidden
    }
  }

  pub fn clear(&mut self, level: Precedence) -> Change {
    let winner = self.effective_source();
    let index = Self::value_index(level).expect("only value levels can be cleared");
    if self.values[index].take().is_some() && level == winner {
      self.visible_change()
    } else {
      Change::Hidden
    }
  }

  pub fn inherited(&self) -> Option<Property<T>> {
    self.inherited
  }

  pub fn set_inherited(&mut self, source: Option<Property<T>>) -> Change {
    if self.inherited == source {
      return Change::Hidden;
    }
    let winner = self.effective_source();
    self.inherited = source;
    if winner <= Precedence::Inherited {
      self.visible_change()
    } else {
      Change::Hidden
    }
  }

  pub fn followers(&self) -> &[Property<T>] {
    &self.followers
  }

  pub fn add_follower(&mut self, follower: Property<T>) {
    self.followers.push(follower);
  }

  pub fn remove_follower(&mut self, follower: Property<T>) {
    self.followers.retain(|existing| *existing != follower);
  }

  pub fn retain_followers(&mut self, keep: impl FnMut(&Property<T>) -> bool) {
    self.followers.retain(keep);
  }

  pub fn mark_dirty(&self) {
    if let Some(dirty) = &self.dirty {
      dirty.set(dirty.get() | self.key.affects());
    }
  }

  fn visible_change(&self) -> Change {
    self.mark_dirty();
    Change::Visible
  }

  fn value_index(level: Precedence) -> Option<usize> {
    (level as usize).checked_sub(Precedence::StyleSetter as usize)
  }

  fn level_at(index: usize) -> Precedence {
    [
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

  static TEST: PropertyKey<i32> = PropertyKey::new("Test", Affects::NONE, false);

  /// Returns the effective value of `levels`.
  fn effective(levels: &Levels<i32>) -> i32 {
    levels.with_effective(|value| *value)
  }

  // Each value level maps to its own slot and back; Default and Inherited have no slot.
  #[test]
  fn value_index_round_trips() {
    for level in [
      Precedence::StyleSetter,
      Precedence::Binding,
      Precedence::Local,
      Precedence::Animation,
    ] {
      let index = Levels::<i32>::value_index(level).expect("a value level has a slot");
      assert_eq!(Levels::<i32>::level_at(index), level);
    }
    assert_eq!(Levels::<i32>::value_index(Precedence::Default), None);
    assert_eq!(Levels::<i32>::value_index(Precedence::Inherited), None);
  }

  // Writing the winning level or a level above it is visible; writing below it is hidden.
  #[test]
  fn set_is_visible_only_at_or_above_the_winner() {
    let mut levels = Levels::new(&TEST, 0);
    assert_eq!(levels.set(Precedence::Binding, 1), Change::Visible);

    assert_eq!(levels.set(Precedence::StyleSetter, 2), Change::Hidden);
    assert_eq!(levels.set(Precedence::Binding, 3), Change::Visible);
    assert_eq!(levels.set(Precedence::Local, 3), Change::Visible);
    assert_eq!(effective(&levels), 3);
    assert_eq!(levels.effective_source(), Precedence::Local);
  }

  // Writing the value a level already holds stores nothing and is hidden.
  #[test]
  fn set_with_the_same_value_is_hidden() {
    let mut levels = Levels::new(&TEST, 0);
    levels.set(Precedence::Local, 1);

    assert_eq!(levels.set(Precedence::Local, 1), Change::Hidden);
    assert_eq!(levels.set(Precedence::Default, 0), Change::Hidden);
  }

  // Writing the default is visible only while no level above it is set.
  #[test]
  fn set_default_is_visible_only_when_nothing_overrides_it() {
    let mut levels = Levels::new(&TEST, 0);
    assert_eq!(levels.set(Precedence::Default, 1), Change::Visible);

    levels.set(Precedence::StyleSetter, 2);

    assert_eq!(levels.set(Precedence::Default, 3), Change::Hidden);
    assert_eq!(effective(&levels), 2);
  }

  // Clearing the winning level is visible and reveals the next one; clearing any other level is
  // hidden.
  #[test]
  fn clear_is_visible_only_for_the_winner() {
    let mut levels = Levels::new(&TEST, 0);
    levels.set(Precedence::StyleSetter, 1);
    levels.set(Precedence::Local, 2);

    assert_eq!(levels.clear(Precedence::Binding), Change::Hidden);
    assert_eq!(levels.clear(Precedence::StyleSetter), Change::Hidden);
    assert_eq!(levels.clear(Precedence::Local), Change::Visible);
    assert_eq!(effective(&levels), 0);
    assert_eq!(levels.effective_source(), Precedence::Default);
  }
}
