use crate::reactive::runtime::{ObserverId, Runtime, SignalId};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ObservableId {
  Signal(SignalId),
}

impl ObservableId {
  pub(crate) fn unsubscribe(self, observer_id: ObserverId) {
    match self {
      ObservableId::Signal(signal_id) => Runtime::unsubscribe(signal_id, observer_id),
    }
  }
}
