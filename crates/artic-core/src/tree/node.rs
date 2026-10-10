//! What one slot of the [`TreeArena`](crate::tree::TreeArena) holds.
//!
//! A [`TreeNode`] keeps a node's object. It's plain storage: the other files of `tree` give it its
//! behavior.

use crate::Object;

#[derive(Debug)]
pub struct TreeNode {
  pub object: Box<dyn Object>,
}

impl TreeNode {
  pub fn new(object: Box<dyn Object>) -> Self {
    Self { object }
  }
}
