//! The XAML infoset: the tree both XAML back ends read, with no trace of XML syntax.
//!
//! An infoset keeps what the markup means and forgets how it was written: `Content="Hi"`,
//! `<Button.Content>Hi</Button.Content>` and `<Button>Hi</Button>` all become an object with one
//! member holding the text. Every piece keeps the [`Span`] it came from, so later stages can point
//! at it. Type references stay unresolved: turning `p:Name` into a Rust type needs the registry,
//! which this crate doesn't depend on.

use crate::diagnostics::{SourceId, Span};

/// A parsed XAML file.
#[derive(Debug, Clone, PartialEq)]
pub struct XamlDocument {
  /// The file, in the [`SourceMap`](crate::diagnostics::SourceMap) it was parsed from.
  pub source: SourceId,
  /// The root element, always an [`XamlNode::Object`].
  pub root: XamlNode,
}

/// One node of the infoset.
#[derive(Debug, Clone, PartialEq)]
pub enum XamlNode {
  /// An object element, such as `<Button Width="100"/>`: "create a `Button`".
  Object {
    /// The element's type, unresolved.
    ty: TypeRef,
    /// The object's members: attributes in order, then property elements in order, then the
    /// content, if any, as one member.
    members: Vec<Member>,
    /// The namespace prefixes this element declares with `xmlns` attributes.
    namespaces: Vec<NamespaceDecl>,
    /// The whole element, from `<` to its end tag.
    span: Span,
  },
  /// Text: an attribute value, or text and CDATA inside an element. Entities are decoded, and
  /// whitespace is kept as written.
  Value {
    /// The text.
    text: String,
    /// Where it was written; its length can differ from `text`'s when entities were decoded.
    span: Span,
  },
  /// A markup extension, such as `{Binding Name}`. Attribute text in braces is parsed into this by
  /// the markup extension grammar; until then it stays a [`XamlNode::Value`].
  MarkupExtension {
    /// The extension's type, unresolved.
    ty: TypeRef,
    /// Arguments given by position.
    positional: Vec<XamlNode>,
    /// Arguments given by name.
    named: Vec<(String, XamlNode)>,
    /// The whole extension, braces included.
    span: Span,
  },
}

impl XamlNode {
  /// Where the node was written.
  #[must_use]
  pub fn span(&self) -> Span {
    match self {
      XamlNode::Object { span, .. } | XamlNode::Value { span, .. } | XamlNode::MarkupExtension { span, .. } => *span,
    }
  }
}

/// One member of an object: which member, and the nodes it holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Member {
  /// Which member.
  pub key: MemberKey,
  /// What it holds, in document order: one value for an attribute, any number of nodes for a
  /// property element or for content.
  pub value: Vec<XamlNode>,
  /// Where the member is named: the attribute's name, or the property element's start tag. For
  /// content, the content itself.
  pub span: Span,
}

/// Which member of an object a [`Member`] sets.
#[derive(Debug, Clone, PartialEq)]
pub enum MemberKey {
  /// A property of the object's own type, such as `Width`.
  Property(String),
  /// A property that another type owns, set on this object, such as `Grid.Row`.
  Attached {
    /// The type that declares the property, unresolved.
    owner: TypeRef,
    /// The property's name.
    name: String,
  },
  /// An instruction to the XAML processor rather than a property, such as `x:Name`.
  Directive(Directive),
  /// Text and child objects written directly inside the element, in document order.
  Content,
  /// Children of a collection. The parser never produces it: whether content goes into a
  /// collection depends on the registry, so the back ends decide.
  Items,
}

/// An `x:` directive, or `xml:space`.
///
/// Directives are recognized by their prefix here; checking that the prefix really maps to the
/// markup namespace happens with namespace resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Directive {
  /// `x:Name`
  Name,
  /// `x:Key`
  Key,
  /// `x:Class`
  Class,
  /// `x:DataType`
  DataType,
  /// `x:Uid`
  Uid,
  /// `x:FieldModifier`
  FieldModifier,
  /// `xml:space`
  XmlSpace,
  /// Any other `x:` name, kept so a later stage can reject it with a precise message.
  Other(String),
}

/// A type named in markup, not yet resolved to a Rust type.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeRef {
  /// The namespace prefix, such as `local` in `local:Gauge`, or `None` for the default namespace.
  pub prefix: Option<String>,
  /// The type's name.
  pub name: String,
  /// Where the name was written.
  pub span: Span,
}

/// A namespace prefix declared with `xmlns` or `xmlns:prefix`.
#[derive(Debug, Clone, PartialEq)]
pub struct NamespaceDecl {
  /// The declared prefix, or `None` for the default namespace (`xmlns="…"`).
  pub prefix: Option<String>,
  /// The namespace it maps to.
  pub uri: String,
  /// The whole `xmlns…="…"` attribute.
  pub span: Span,
}
