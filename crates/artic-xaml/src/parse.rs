//! Turning XAML text into the infoset.

#[cfg(test)]
mod tests;
mod text;

use crate::diagnostics::{Diagnostic, Diagnostics, LineCol, SourceFile, SourceId, SourceMap, Span};
use crate::infoset::{Directive, Member, MemberKey, NamespaceDecl, TypeRef, XamlDocument, XamlNode};
use text::{Kind, decode, normalize_line_endings};
use xmlparser::{ElementEnd, StrSpan, Token, Tokenizer};

/// Parses the XAML file `file` of `sources` into its infoset.
///
/// # Errors
///
/// Returns every problem found: malformed XML stops parsing at the first error, while misplaced
/// attributes and elements are all collected before returning.
///
/// # Examples
///
/// ```
/// use artic_xaml::diagnostics::SourceMap;
/// use artic_xaml::infoset::{MemberKey, XamlNode};
/// use artic_xaml::parse;
///
/// let mut sources = SourceMap::new();
/// let file = sources.add("view.xaml", r#"<Button Width="100">OK</Button>"#);
/// let document = parse(&sources, file).expect("valid XAML");
/// let XamlNode::Object { ty, members, .. } = &document.root else { unreachable!() };
/// assert_eq!(ty.name, "Button");
/// assert_eq!(members[0].key, MemberKey::Property("Width".to_owned()));
/// assert_eq!(members[1].key, MemberKey::Content);
/// ```
pub fn parse(sources: &SourceMap, file: SourceId) -> Result<XamlDocument, Diagnostics> {
  let source = sources.get(file);
  let mut builder = Builder {
    file,
    source,
    diagnostics: Diagnostics::new(),
    stack: Vec::new(),
    root: None,
  };
  for token in Tokenizer::from(source.text()) {
    let step = match token {
      Ok(token) => builder.token(token),
      Err(error) => {
        let position = error.pos();
        let byte = source.byte_offset(LineCol {
          line: position.row as usize,
          column: position.col as usize,
        });
        let span = builder.span(byte, byte);
        let diagnostic = if builder.stack.is_empty() && builder.root.is_some() {
          Diagnostic::error(span, "expected one root element, found more content after it")
            .with_help("only comments and processing instructions may follow the root element")
        } else {
          Diagnostic::error(span, format!("malformed XML: {error}"))
        };
        Err(builder.fatal(diagnostic))
      }
    };
    if step.is_err() {
      return Err(builder.diagnostics);
    }
  }
  builder.finish()
}

/// An element whose end tag hasn't been reached yet.
struct Open {
  /// The tag name as written, to match the end tag.
  tag: String,
  /// Where the element starts.
  start: usize,
  /// The start tag's name, for "opened here" labels.
  tag_span: Span,
  /// Text and CDATA read since the last element boundary: the decoded text and its span.
  pending_text: Option<(String, usize, usize)>,
  kind: OpenKind,
}

enum OpenKind {
  Object {
    ty: TypeRef,
    members: Vec<Member>,
    namespaces: Vec<NamespaceDecl>,
    content: Vec<XamlNode>,
  },
  PropertyElement {
    key: MemberKey,
    value: Vec<XamlNode>,
  },
}

/// Marks a fatal error, already recorded in [`Builder::diagnostics`]: parsing stops.
struct Fatal;

struct Builder<'a> {
  file: SourceId,
  source: &'a SourceFile,
  diagnostics: Diagnostics,
  stack: Vec<Open>,
  root: Option<XamlNode>,
}

impl Builder<'_> {
  /// Handles one token. A fatal error stops parsing: the document's structure can't be trusted after
  /// it.
  fn token(&mut self, token: Token) -> Result<(), Fatal> {
    match token {
      Token::ElementStart { prefix, local, span } => self.element_start(prefix, local, span),
      Token::Attribute {
        prefix, local, value, ..
      } => {
        self.attribute(prefix, local, value);
        Ok(())
      }
      Token::ElementEnd { end, span } => match end {
        ElementEnd::Open => Ok(()),
        ElementEnd::Empty => self.close(span.end()),
        ElementEnd::Close(prefix, local) => self.end_tag(prefix, local, span),
      },
      Token::Text { text } => {
        self.text(text);
        Ok(())
      }
      Token::Cdata { text, span } => {
        self.add_text(normalize_line_endings(text.as_str()), span.start(), span.end());
        Ok(())
      }
      Token::DtdStart { span, .. } | Token::EmptyDtd { span, .. } => Err(
        self.fatal(
          Diagnostic::error(self.span_of(span), "DTDs aren't supported in XAML")
            .with_help("remove the `<!DOCTYPE …>` declaration"),
        ),
      ),
      Token::Declaration { .. }
      | Token::ProcessingInstruction { .. }
      | Token::Comment { .. }
      | Token::EntityDeclaration { .. }
      | Token::DtdEnd { .. } => Ok(()),
    }
  }

  fn element_start(&mut self, prefix: StrSpan, local: StrSpan, span: StrSpan) -> Result<(), Fatal> {
    self.flush_text();
    let tag_span = self.span(span.start() + 1, local.end());
    let tag = qualified(prefix, local);
    let prefix_text = (!prefix.as_str().is_empty()).then(|| prefix.as_str().to_owned());
    let kind = if let Some((owner, name)) = local.as_str().split_once('.') {
      let Some(parent) = self.stack.last() else {
        return Err(self.fatal(Diagnostic::error(tag_span, "the root element can't be a property element")));
      };
      let OpenKind::Object { ty, .. } = &parent.kind else {
        return Err(
          self.fatal(
            Diagnostic::error(tag_span, "a property element can't contain a property element")
              .with_secondary(parent.tag_span, "inside this property element"),
          ),
        );
      };
      let owner_span = self.span(local.start(), local.start() + owner.len());
      let key = if ty.prefix == prefix_text && ty.name == owner {
        MemberKey::Property(name.to_owned())
      } else {
        MemberKey::Attached {
          owner: TypeRef {
            prefix: prefix_text,
            name: owner.to_owned(),
            span: self.span(span.start() + 1, owner_span.end),
          },
          name: name.to_owned(),
        }
      };
      OpenKind::PropertyElement { key, value: Vec::new() }
    } else {
      OpenKind::Object {
        ty: TypeRef {
          prefix: prefix_text,
          name: local.as_str().to_owned(),
          span: tag_span,
        },
        members: Vec::new(),
        namespaces: Vec::new(),
        content: Vec::new(),
      }
    };
    self.stack.push(Open {
      tag,
      start: span.start(),
      tag_span,
      pending_text: None,
      kind,
    });
    Ok(())
  }

  fn attribute(&mut self, prefix: StrSpan, local: StrSpan, value: StrSpan) {
    let name_start = if prefix.as_str().is_empty() {
      local.start()
    } else {
      prefix.start()
    };
    let name_span = self.span(name_start, local.end());
    let value_text = match decode(value.as_str(), value.start(), Kind::Attribute) {
      Ok(text) => text,
      Err(error) => {
        let span = self.span(error.start, error.end);
        self.diagnostics.push(Diagnostic::error(span, error.message));
        return;
      }
    };
    let value_node = XamlNode::Value {
      text: value_text.clone(),
      span: self.span_of(value),
    };
    let file = self.file;
    let open = self
      .stack
      .last_mut()
      .expect("the tokenizer reads attributes only in a start tag");
    let tag_span = open.tag_span;
    let OpenKind::Object {
      ty, members, namespaces, ..
    } = &mut open.kind
    else {
      self.diagnostics.push(
        Diagnostic::error(name_span, "a property element can't have attributes")
          .with_secondary(tag_span, "on this property element"),
      );
      return;
    };
    let (prefix, local) = (prefix.as_str(), local.as_str());
    let key = match (prefix, local) {
      ("", "xmlns") | ("xmlns", _) => {
        namespaces.push(NamespaceDecl {
          prefix: (prefix == "xmlns").then(|| local.to_owned()),
          uri: value_text,
          span: Span::new(file, name_span.start, value.end() + 1),
        });
        return;
      }
      ("xml", "space") => MemberKey::Directive(Directive::XmlSpace),
      ("x", name) => MemberKey::Directive(directive(name)),
      (_, name) if name.contains('.') => {
        let (owner, property) = name.split_once('.').expect("contains a dot");
        let owner_prefix = (!prefix.is_empty()).then(|| prefix.to_owned());
        if ty.prefix == owner_prefix && ty.name == owner {
          MemberKey::Property(property.to_owned())
        } else {
          MemberKey::Attached {
            owner: TypeRef {
              prefix: owner_prefix,
              name: owner.to_owned(),
              span: Span::new(file, name_span.start, name_span.start + prefix_len(prefix) + owner.len()),
            },
            name: property.to_owned(),
          }
        }
      }
      ("", name) => MemberKey::Property(name.to_owned()),
      (prefix, name) => {
        self.diagnostics.push(
          Diagnostic::error(name_span, format!("`{prefix}:{name}` names no property")).with_help(format!(
            "a prefixed attribute must name an attached property, like `{prefix}:Owner.{name}`"
          )),
        );
        return;
      }
    };
    let member = Member {
      key,
      value: vec![value_node],
      span: name_span,
    };
    if let Some(duplicate) = duplicate_of(members, &member) {
      self.diagnostics.push(duplicate);
      return;
    }
    members.push(member);
  }

  fn end_tag(&mut self, prefix: StrSpan, local: StrSpan, span: StrSpan) -> Result<(), Fatal> {
    let tag = qualified(prefix, local);
    let open = self
      .stack
      .last()
      .expect("the tokenizer reads end tags only inside an element");
    if open.tag != tag {
      return Err(
        self.fatal(
          Diagnostic::error(self.span_of(span), format!("expected `</{}>`, found `</{tag}>`", open.tag))
            .with_secondary(open.tag_span, "opened here"),
        ),
      );
    }
    self.close(span.end())
  }

  /// Closes the innermost open element, which ends at byte `end`, and hands it to its parent.
  fn close(&mut self, end: usize) -> Result<(), Fatal> {
    self.flush_text();
    let open = self.stack.pop().expect("an element is open when one closes");
    let span = self.span(open.start, end);
    match open.kind {
      OpenKind::Object {
        ty,
        mut members,
        namespaces,
        content,
      } => {
        if let (Some(first), Some(last)) = (content.first(), content.last()) {
          let content_span = self.span(first.span().start, last.span().end);
          members.push(Member {
            key: MemberKey::Content,
            value: content,
            span: content_span,
          });
        }
        let node = XamlNode::Object {
          ty,
          members,
          namespaces,
          span,
        };
        match self.stack.last_mut() {
          Some(Open {
            kind: OpenKind::Object { content, .. },
            ..
          }) => content.push(node),
          Some(Open {
            kind: OpenKind::PropertyElement { value, .. },
            ..
          }) => value.push(node),
          None => self.root = Some(node),
        }
      }
      OpenKind::PropertyElement { key, value } => {
        let member = Member {
          key,
          value,
          span: open.tag_span,
        };
        let Some(Open {
          kind: OpenKind::Object { members, .. },
          ..
        }) = self.stack.last_mut()
        else {
          unreachable!("a property element is always opened inside an object element");
        };
        match duplicate_of(members, &member) {
          Some(duplicate) => self.diagnostics.push(duplicate),
          None => members.push(member),
        }
      }
    }
    Ok(())
  }

  fn text(&mut self, text: StrSpan) {
    match decode(text.as_str(), text.start(), Kind::Text) {
      Ok(decoded) => self.add_text(decoded, text.start(), text.end()),
      Err(error) => {
        let span = self.span(error.start, error.end);
        self.diagnostics.push(Diagnostic::error(span, error.message));
      }
    }
  }

  /// Adds decoded text, written at bytes `start..end`, to the innermost element's pending text.
  fn add_text(&mut self, text: String, start: usize, end: usize) {
    let open = self
      .stack
      .last_mut()
      .expect("the tokenizer reads text only inside an element");
    match &mut open.pending_text {
      Some((pending, _, pending_end)) => {
        pending.push_str(&text);
        *pending_end = end;
      }
      None => open.pending_text = Some((text, start, end)),
    }
  }

  /// Moves the innermost element's pending text into its content or value.
  fn flush_text(&mut self) {
    let file = self.file;
    let Some(open) = self.stack.last_mut() else {
      return;
    };
    let Some((text, start, end)) = open.pending_text.take() else {
      return;
    };
    let node = XamlNode::Value {
      text,
      span: Span::new(file, start, end),
    };
    match &mut open.kind {
      OpenKind::Object { content, .. } => content.push(node),
      OpenKind::PropertyElement { value, .. } => value.push(node),
    }
  }

  fn finish(mut self) -> Result<XamlDocument, Diagnostics> {
    let end = self.source.text().len();
    if let Some(open) = self.stack.last() {
      let diagnostic = Diagnostic::error(self.span(end, end), format!("`<{}>` is never closed", open.tag))
        .with_secondary(open.tag_span, "opened here");
      self.diagnostics.push(diagnostic);
      return Err(self.diagnostics);
    }
    let Some(root) = self.root else {
      self
        .diagnostics
        .push(Diagnostic::error(self.span(0, end), "expected one root element"));
      return Err(self.diagnostics);
    };
    if !self.diagnostics.is_empty() {
      return Err(self.diagnostics);
    }
    Ok(XamlDocument { source: self.file, root })
  }

  fn fatal(&mut self, diagnostic: Diagnostic) -> Fatal {
    self.diagnostics.push(diagnostic);
    Fatal
  }

  fn span(&self, start: usize, end: usize) -> Span {
    Span::new(self.file, start, end)
  }

  fn span_of(&self, text: StrSpan) -> Span {
    self.span(text.start(), text.end())
  }
}

/// A tag name as written: `prefix:local` or `local`.
fn qualified(prefix: StrSpan, local: StrSpan) -> String {
  if prefix.as_str().is_empty() {
    local.as_str().to_owned()
  } else {
    format!("{}:{}", prefix.as_str(), local.as_str())
  }
}

/// The length of `prefix:` in front of a name, or 0 without a prefix.
fn prefix_len(prefix: &str) -> usize {
  if prefix.is_empty() { 0 } else { prefix.len() + 1 }
}

fn directive(name: &str) -> Directive {
  match name {
    "Name" => Directive::Name,
    "Key" => Directive::Key,
    "Class" => Directive::Class,
    "DataType" => Directive::DataType,
    "Uid" => Directive::Uid,
    "FieldModifier" => Directive::FieldModifier,
    other => Directive::Other(other.to_owned()),
  }
}

/// An error if `members` already sets `member`'s key, pointing at both.
fn duplicate_of(members: &[Member], member: &Member) -> Option<Diagnostic> {
  let first = members.iter().find(|existing| same_member(&existing.key, &member.key))?;
  Some(
    Diagnostic::error(member.span, "this member is set twice on the same element")
      .with_label("set again here")
      .with_secondary(first.span, "first set here"),
  )
}

/// Whether two keys name the same member, wherever their owner types are written.
fn same_member(a: &MemberKey, b: &MemberKey) -> bool {
  match (a, b) {
    (
      MemberKey::Attached {
        owner: a_owner,
        name: a_name,
      },
      MemberKey::Attached {
        owner: b_owner,
        name: b_name,
      },
    ) => a_owner.prefix == b_owner.prefix && a_owner.name == b_owner.name && a_name == b_name,
    _ => a == b,
  }
}
