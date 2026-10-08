use crate::reactive::runtime::{MemoId, ObserverId, RUNTIME, Runtime, SignalId};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ObservableId {
  Signal(SignalId),
  Memo(MemoId),
}

impl ObservableId {
  pub fn refresh(self) {
    match self {
      ObservableId::Signal(_) => {}
      ObservableId::Memo(memo_id) => {
        if RUNTIME.with(|runtime| runtime.memos.contains(memo_id)) {
          Runtime::refresh_memo(memo_id);
        }
      }
    }
  }

  pub fn unsubscribe(self, observer_id: ObserverId) {
    let signal_id = match self {
      ObservableId::Signal(signal_id) => signal_id,
      ObservableId::Memo(memo_id) => {
        if !RUNTIME.with(|runtime| runtime.memos.contains(memo_id)) {
          return;
        }
        Runtime::memo_value(memo_id)
      }
    };
    RUNTIME.with(|runtime| runtime.signals.remove_subscriber(signal_id, observer_id));
  }
}
