use crate::Object;
use crate::tree::{NodeId, Tree};

#[derive(Debug)]
pub struct TreeNode {
  pub object: Box<dyn Object>,
  logical: Links,
  visual: Links,
}

#[derive(Debug, Default)]
pub struct Links {
  pub parent: Option<NodeId>,
  pub children: Vec<NodeId>,
}

impl TreeNode {
  pub fn new(object: Box<dyn Object>) -> Self {
    Self {
      object,
      logical: Links::default(),
      visual: Links::default(),
    }
  }

  pub fn links(&self, tree: Tree) -> &Links {
    match tree {
      Tree::Logical => &self.logical,
      Tree::Visual => &self.visual,
    }
  }

  pub fn links_mut(&mut self, tree: Tree) -> &mut Links {
    match tree {
      Tree::Logical => &mut self.logical,
      Tree::Visual => &mut self.visual,
    }
  }
}
