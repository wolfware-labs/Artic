use crate::Object;
use crate::property::{Affects, AnyProperty};
use crate::tree::{NodeId, Tree};
use std::cell::Cell;
use std::rc::Rc;

#[derive(Debug)]
pub struct TreeNode {
  pub object: Box<dyn Object>,
  pub dirty: Rc<Cell<Affects>>,
  pub attached: Vec<Box<dyn AnyProperty>>,
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
    let dirty = Rc::new(Cell::new(Affects::LAYOUT | Affects::RENDER));
    object.visit_properties(&mut |property| property.attach(Rc::clone(&dirty)));
    Self {
      object,
      dirty,
      attached: Vec::new(),
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
