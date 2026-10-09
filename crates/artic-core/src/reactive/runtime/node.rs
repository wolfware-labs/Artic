use crate::reactive::runtime::{EffectId, MemoId, Runtime, SignalId};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Node {
  Signal(SignalId),
  Memo(MemoId),
  Effect(EffectId),
}

impl Node {
  pub fn refresh(self) {
    match self {
      Node::Signal(_) | Node::Effect(_) => {}
      Node::Memo(memo_id) => Runtime::refresh_memo(memo_id),
    }
  }
}
