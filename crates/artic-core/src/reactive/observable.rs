use crate::reactive::memo;
use crate::reactive::runtime::{ObservableId, ObserverId, Runtime};

impl ObservableId {
  pub(crate) fn refresh(self) {
    match self {
      ObservableId::Signal(_) => {}
      ObservableId::Memo(memo_id) => memo::refresh(memo_id),
    }
  }

  pub(crate) fn unsubscribe(self, observer_id: ObserverId) {
    match self {
      ObservableId::Signal(signal_id) => Runtime::unsubscribe(signal_id, observer_id),
      ObservableId::Memo(memo_id) => Runtime::unsubscribe(Runtime::memo_value(memo_id), observer_id),
    }
  }
}
