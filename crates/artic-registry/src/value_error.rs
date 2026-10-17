//! Why markup's value or child was refused: the errors of the runtime loader
//! ([loader design](https://github.com/wolfware-labs/artic/blob/main/docs/design/loader-values.md)),
//! which the content route already returns.

use artic_core::tree::LinkError;

/// Why a value or a child element from markup was refused. Positions are added by the loader.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ValueError {
  /// The text didn't parse. `message` is the value type's `FromStr` error, displayed.
  #[error("expected {expected}: {message}")]
  Parse {
    /// The type the text should have parsed as.
    expected: &'static str,
    /// The parser's message.
    message: String,
  },
  /// `x:Null` on a property whose type isn't `Option<T>`.
  #[error("{property} can't be x:Null")]
  NotNullable {
    /// The property's name.
    property: &'static str,
  },
  /// An object or resource of the wrong type.
  #[error("expected {expected}, found {found}")]
  WrongType {
    /// The type the property takes.
    expected: &'static str,
    /// The type it was given.
    found: &'static str,
  },
  /// The target node isn't of the property's or the content's owner type.
  #[error("the node isn't of the property's or content's owner type")]
  WrongNode,
  /// The child can't be linked to its parent.
  #[error("can't add the child: {0}")]
  InvalidChild(#[source] LinkError),
  /// A second child for an element that takes one.
  #[error("this element takes a single child")]
  OneChildOnly,
}
