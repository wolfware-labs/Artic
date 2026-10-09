use crate::reactive::runtime::{EffectId, MemoId, ObserverId, RUNTIME, Runtime, SignalId};

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

  pub fn unsubscribe(self, observer_id: ObserverId) {
    let signal_id = match self {
      Node::Signal(signal_id) => signal_id,
      Node::Effect(_) => unreachable!("nothing reads an effect"),
      Node::Memo(memo_id) => {
        let Some(value_id) = RUNTIME.with(|runtime| runtime.memos.find_value(memo_id)) else {
          return;
        };
        value_id
      }
    };
    RUNTIME.with(|runtime| runtime.signals.remove_subscriber(signal_id, observer_id));
  }
}
