//! Decoding XML text and attribute values.

/// Why a piece of text couldn't be decoded, and where, as byte offsets into the source.
#[derive(Debug, PartialEq)]
pub struct DecodeError {
  pub start: usize,
  pub end: usize,
  pub message: String,
}

/// How whitespace in the raw text is treated.
#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
  /// Element text: line endings become `\n`, other whitespace is kept.
  Text,
  /// An attribute value: line endings, tabs and newlines each become one space (XML 1.0 §3.3.3).
  Attribute,
}

/// Decodes `raw`, which starts at byte `offset` of the source: resolves the five predefined
/// entities and character references, and normalizes line endings (and, for attributes,
/// whitespace) the way an XML processor does. Characters written as references are never
/// normalized.
pub fn decode(raw: &str, offset: usize, kind: Kind) -> Result<String, DecodeError> {
  let mut decoded = String::with_capacity(raw.len());
  let mut rest = raw.char_indices().peekable();
  while let Some((index, character)) = rest.next() {
    match character {
      '&' => {
        let end = raw[index..].find(';').map(|position| index + position + 1);
        let Some(end) = end else {
          return Err(DecodeError {
            start: offset + index,
            end: offset + index + 1,
            message: "`&` starts an entity that has no closing `;`; write `&amp;` for a literal `&`".to_owned(),
          });
        };
        decoded.push(resolve(&raw[index..end], offset + index)?);
        while rest.peek().is_some_and(|(next, _)| *next < end) {
          rest.next();
        }
      }
      '\r' => {
        if rest.peek().is_some_and(|(_, next)| *next == '\n') {
          rest.next();
        }
        decoded.push(if kind == Kind::Attribute { ' ' } else { '\n' });
      }
      '\n' | '\t' if kind == Kind::Attribute => decoded.push(' '),
      _ => decoded.push(character),
    }
  }
  Ok(decoded)
}

/// Normalizes line endings in CDATA text, which has no entities.
pub fn normalize_line_endings(raw: &str) -> String {
  raw.replace("\r\n", "\n").replace('\r', "\n")
}

/// Resolves one `&…;` reference, which starts at byte `start` of the source.
fn resolve(reference: &str, start: usize) -> Result<char, DecodeError> {
  let name = &reference[1..reference.len() - 1];
  let error = |message: String| DecodeError {
    start,
    end: start + reference.len(),
    message,
  };
  let character = match name {
    "lt" => Some('<'),
    "gt" => Some('>'),
    "amp" => Some('&'),
    "quot" => Some('"'),
    "apos" => Some('\''),
    _ => {
      let code = if let Some(hex) = name.strip_prefix("#x") {
        u32::from_str_radix(hex, 16).ok()
      } else if let Some(decimal) = name.strip_prefix('#') {
        decimal.parse().ok()
      } else {
        return Err(error(format!(
          "unknown entity `{reference}`; XML only defines `&lt;`, `&gt;`, `&amp;`, `&quot;` and `&apos;`, so write other characters as references like `&#160;`"
        )));
      };
      code.and_then(char::from_u32)
    }
  };
  character.ok_or_else(|| error(format!("`{reference}` is not a valid character reference")))
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;

  // The predefined entities and character references are resolved.
  #[test]
  fn entities_and_references_are_resolved() {
    assert_eq!(
      decode("a &lt; b &amp;&amp; &#65;&#x42;", 0, Kind::Text),
      Ok("a < b && AB".to_owned())
    );
  }

  // Line endings become `\n` in text; in attributes every whitespace character becomes a space,
  // except characters written as references.
  #[test]
  fn whitespace_normalization() {
    assert_eq!(decode("a\r\nb\rc\td", 0, Kind::Text), Ok("a\nb\nc\td".to_owned()));
    assert_eq!(decode("a\r\nb\tc\nd&#10;e", 0, Kind::Attribute), Ok("a b c d\ne".to_owned()));
  }

  // An unknown entity is reported at its position in the source, with a hint.
  #[test]
  fn unknown_entity_is_an_error() {
    let error = decode("a &nbsp; b", 10, Kind::Text).expect_err("unknown entity");

    assert_eq!((error.start, error.end), (12, 18));
    assert!(error.message.starts_with("unknown entity `&nbsp;`"));
  }

  // A bare `&` and an invalid code point are rejected.
  #[test]
  fn bad_references_are_errors() {
    assert!(decode("a & b", 0, Kind::Text).is_err());
    assert!(decode("&#xD800;", 0, Kind::Text).is_err());
  }
}
