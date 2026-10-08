use crate::reactive::memo::{MemoId, MemoStore};
use crate::reactive::runtime::{ObserverId, Runtime, SignalId};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ObservableId {
  Signal(SignalId),
  Memo(MemoId),
}

impl ObservableId {
  pub(crate) fn refresh(self) {
    match self {
      ObservableId::Signal(_) => {}
      ObservableId::Memo(memo_id) => MemoStore::refresh(memo_id),
    }
  }

  pub(crate) fn unsubscribe(self, observer_id: ObserverId) {
    match self {
      ObservableId::Signal(signal_id) => Runtime::unsubscribe(signal_id, observer_id),
      ObservableId::Memo(memo_id) => Runtime::unsubscribe(MemoStore::value(memo_id), observer_id),
    }
  }
}
