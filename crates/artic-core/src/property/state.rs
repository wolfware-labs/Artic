use crate::property::levels::Levels;
use crate::property::{Affects, Property, PropertyKey};
use std::cell::Cell;
use std::rc::Rc;

#[derive(Debug)]
pub struct PropertyState<T>
where
  T: 'static,
{
  pub levels: Levels<T>,
  key: &'static PropertyKey<T>,
  dirty: Option<Rc<Cell<Affects>>>,
  followers: Vec<Property<T>>,
}

impl<T> PropertyState<T>
where
  T: PartialEq,
{
  pub fn new(key: &'static PropertyKey<T>, default: T) -> Self {
    Self {
      levels: Levels::new(default),
      key,
      dirty: None,
      followers: Vec::new(),
    }
  }

  pub fn key(&self) -> &'static PropertyKey<T> {
    self.key
  }

  pub fn attach(&mut self, dirty: Rc<Cell<Affects>>) {
    self.dirty = Some(dirty);
  }

  pub fn mark_dirty(&self) {
    if let Some(dirty) = &self.dirty {
      dirty.set(dirty.get() | self.key.affects());
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
}
