//! Font families, weights and styles, and parsing them from XAML text.

use std::str::FromStr;
use std::sync::Arc;

/// A list of font families, tried in order: CSS's `font-family`.
///
/// It parses from a comma-separated list such as `'Segoe UI', Inter, sans-serif`. Each name is
/// quoted with `'` or `"`, or not. An unquoted name that is a generic keyword, ignoring case, is
/// that [`GenericFamily`]; a quoted one is a font with that name. The default is `system-ui`.
///
/// # Examples
///
/// ```
/// use artic_layout::text::{FamilyName, FontFamily, GenericFamily};
///
/// let family: FontFamily = "'Segoe UI', sans-serif".parse().unwrap();
/// assert_eq!(
///   family.names(),
///   [FamilyName::named("Segoe UI"), FamilyName::Generic(GenericFamily::SansSerif)]
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FontFamily(Arc<[FamilyName]>);

impl FontFamily {
  /// The families, in the order they're tried.
  #[must_use]
  pub fn names(&self) -> &[FamilyName] {
    &self.0
  }
}

impl Default for FontFamily {
  /// `system-ui`.
  fn default() -> FontFamily {
    FontFamily::from(GenericFamily::SystemUi)
  }
}

impl From<GenericFamily> for FontFamily {
  /// A list of that generic family alone.
  fn from(generic: GenericFamily) -> FontFamily {
    FontFamily(Arc::from([FamilyName::Generic(generic)]))
  }
}

/// One entry of a [`FontFamily`]: a font's name or a generic family.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FamilyName {
  /// A font family's name, such as `Inter`.
  Named(Arc<str>),
  /// A generic family, which the text engine maps to one of its fonts.
  Generic(GenericFamily),
}

impl FamilyName {
  /// The family named `name`.
  #[must_use]
  pub fn named(name: &str) -> FamilyName {
    FamilyName::Named(Arc::from(name))
  }
}

/// A CSS generic font family: a kind of font, which the text engine maps to one of its fonts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GenericFamily {
  /// `serif`: strokes end in small flourishes.
  Serif,
  /// `sans-serif`: plain stroke ends.
  SansSerif,
  /// `monospace`: every glyph is as wide as the others.
  Monospace,
  /// `cursive`: joined, handwritten strokes.
  Cursive,
  /// `fantasy`: decorative.
  Fantasy,
  /// `system-ui`: the desktop's own interface font.
  SystemUi,
  /// `emoji`: color emoji.
  Emoji,
  /// `math`: mathematical notation.
  Math,
}

impl GenericFamily {
  /// Every generic family, in CSS's order.
  const ALL: [GenericFamily; 8] = [
    GenericFamily::Serif,
    GenericFamily::SansSerif,
    GenericFamily::Monospace,
    GenericFamily::Cursive,
    GenericFamily::Fantasy,
    GenericFamily::SystemUi,
    GenericFamily::Emoji,
    GenericFamily::Math,
  ];

  /// The family's CSS keyword, such as `sans-serif`.
  #[must_use]
  pub const fn keyword(self) -> &'static str {
    match self {
      GenericFamily::Serif => "serif",
      GenericFamily::SansSerif => "sans-serif",
      GenericFamily::Monospace => "monospace",
      GenericFamily::Cursive => "cursive",
      GenericFamily::Fantasy => "fantasy",
      GenericFamily::SystemUi => "system-ui",
      GenericFamily::Emoji => "emoji",
      GenericFamily::Math => "math",
    }
  }
}

/// The error returned when text isn't a valid font family list.
///
/// The message repeats the text, says what is wrong, and shows the accepted forms.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid FontFamily '{text}': {problem}; expected forms like 'Inter, sans-serif' or 'system-ui'")]
pub struct ParseFontFamilyError {
  /// The text that failed to parse.
  pub text: String,
  /// What is wrong with it.
  pub problem: FamilyProblem,
}

/// What is wrong with font family text.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FamilyProblem {
  /// The text is empty, or only whitespace.
  #[error("it's empty")]
  Empty,
  /// A name between commas is empty, as in `Inter,,Arial`, or a quoted name is, as in `''`.
  #[error("a name is empty")]
  EmptyName,
  /// A quote isn't closed, as in `'Segoe UI`.
  #[error("a quote isn't closed")]
  UnclosedQuote,
  /// Something other than a comma follows a quoted name, as `UI` does in `'Segoe' UI`.
  #[error("'{0}' follows a quoted name")]
  AfterQuote(String),
}

impl FromStr for FontFamily {
  type Err = ParseFontFamilyError;

  /// Parses a comma-separated list of names, each quoted with `'` or `"` or not. Whitespace around
  /// a name is ignored, and whitespace inside an unquoted name becomes one space.
  fn from_str(text: &str) -> Result<FontFamily, ParseFontFamilyError> {
    parse_family(text)
      .map(|names| FontFamily(names.into()))
      .map_err(|problem| ParseFontFamilyError {
        text: text.to_owned(),
        problem,
      })
  }
}

fn parse_family(text: &str) -> Result<Vec<FamilyName>, FamilyProblem> {
  if text.trim().is_empty() {
    return Err(FamilyProblem::Empty);
  }
  let mut names = Vec::new();
  let mut rest = Some(text);
  while let Some(entry) = rest {
    let entry = entry.trim_start();
    let (name, after) = match entry.chars().next() {
      Some(quote @ ('\'' | '"')) => parse_quoted(&entry[1..], quote)?,
      _ => parse_unquoted(entry)?,
    };
    names.push(name);
    rest = after;
  }
  Ok(names)
}

fn parse_quoted(text: &str, quote: char) -> Result<(FamilyName, Option<&str>), FamilyProblem> {
  let (name, after) = text.split_once(quote).ok_or(FamilyProblem::UnclosedQuote)?;
  if name.trim().is_empty() {
    return Err(FamilyProblem::EmptyName);
  }
  let after = after.trim_start();
  let rest = match after.strip_prefix(',') {
    Some(rest) => Some(rest),
    None if after.is_empty() => None,
    None => {
      let stray = after.split(',').next().unwrap_or(after).trim_end();
      return Err(FamilyProblem::AfterQuote(stray.to_owned()));
    }
  };
  Ok((FamilyName::named(name), rest))
}

fn parse_unquoted(text: &str) -> Result<(FamilyName, Option<&str>), FamilyProblem> {
  let (name, rest) = match text.split_once(',') {
    Some((name, rest)) => (name, Some(rest)),
    None => (text, None),
  };
  let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
  if name.is_empty() {
    return Err(FamilyProblem::EmptyName);
  }
  let generic = GenericFamily::ALL
    .into_iter()
    .find(|generic| generic.keyword().eq_ignore_ascii_case(&name));
  Ok((generic.map_or_else(|| FamilyName::named(&name), FamilyName::Generic), rest))
}

#[cfg(test)]
mod tests {
  use super::*;
  use FamilyName::Generic;
  use GenericFamily::{SansSerif, Serif, SystemUi};
  use pretty_assertions::assert_eq;

  fn names(text: &str) -> Vec<FamilyName> {
    text.parse::<FontFamily>().unwrap().names().to_vec()
  }

  fn problem(text: &str) -> FamilyProblem {
    text.parse::<FontFamily>().unwrap_err().problem
  }

  fn named(name: &str) -> FamilyName {
    FamilyName::named(name)
  }

  // Quoted and unquoted names in one list, with a generic keyword recognized when unquoted.
  #[test]
  fn font_family_parses_list() {
    assert_eq!(
      names(r#"'Segoe UI', "Noto Sans", Inter, sans-serif"#),
      vec![named("Segoe UI"), named("Noto Sans"), named("Inter"), Generic(SansSerif)]
    );
  }

  // Generic keywords match ignoring case, and a quoted keyword is a font with that name.
  #[test]
  fn generic_keywords_ignore_case_unless_quoted() {
    assert_eq!(
      names("SERIF, System-UI, 'monospace'"),
      vec![Generic(Serif), Generic(SystemUi), named("monospace")]
    );
  }

  // Every generic keyword parses to its family.
  #[test]
  fn every_generic_keyword_parses() {
    for generic in GenericFamily::ALL {
      assert_eq!(names(generic.keyword()), vec![Generic(generic)]);
    }
  }

  // Whitespace around names is dropped and runs of it inside an unquoted name become one space;
  // a quoted name keeps its spaces and its commas.
  #[test]
  fn whitespace_and_commas_in_names() {
    assert_eq!(names("  Segoe \t UI ,Arial "), vec![named("Segoe UI"), named("Arial")]);
    assert_eq!(names("'A,  B' , C"), vec![named("A,  B"), named("C")]);
  }

  // The default family is system-ui.
  #[test]
  fn default_family_is_system_ui() {
    assert_eq!(FontFamily::default().names(), [Generic(SystemUi)]);
  }

  // Empty or blank text is `Empty`, and the message quotes it and shows the forms.
  #[test]
  fn empty_family_is_empty() {
    assert_eq!(problem(""), FamilyProblem::Empty);
    assert_eq!(
      "  ".parse::<FontFamily>().unwrap_err().to_string(),
      "invalid FontFamily '  ': it's empty; expected forms like 'Inter, sans-serif' or 'system-ui'"
    );
  }

  // A name missing between commas, before or after them, or an empty quoted name, is `EmptyName`.
  #[test]
  fn missing_name_is_empty_name() {
    for text in ["Inter,,Arial", "Inter,", ",Inter", "''", "Inter, \"\""] {
      assert_eq!(problem(text), FamilyProblem::EmptyName, "{text}");
    }
    assert_eq!(
      "Inter,,Arial".parse::<FontFamily>().unwrap_err().to_string(),
      "invalid FontFamily 'Inter,,Arial': a name is empty; expected forms like 'Inter, sans-serif' or 'system-ui'"
    );
  }

  // A quote without its closing quote, even with the other kind of quote later, is `UnclosedQuote`.
  #[test]
  fn open_quote_is_unclosed_quote() {
    assert_eq!(problem("'Segoe UI"), FamilyProblem::UnclosedQuote);
    assert_eq!(problem(r#"Arial, "Segoe UI'"#), FamilyProblem::UnclosedQuote);
    assert_eq!(
      "'Segoe UI".parse::<FontFamily>().unwrap_err().to_string(),
      "invalid FontFamily ''Segoe UI': a quote isn't closed; expected forms like 'Inter, sans-serif' or 'system-ui'"
    );
  }

  // Text after a closing quote, other than a comma, is `AfterQuote` with the text up to the next comma.
  #[test]
  fn text_after_a_quote_is_reported() {
    assert_eq!(problem("'Segoe' UI , Arial"), FamilyProblem::AfterQuote(String::from("UI")));
    assert_eq!(
      "'Segoe' UI".parse::<FontFamily>().unwrap_err().to_string(),
      "invalid FontFamily ''Segoe' UI': 'UI' follows a quoted name; expected forms like 'Inter, sans-serif' or 'system-ui'"
    );
  }
}
