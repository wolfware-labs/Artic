//! Parsing geometry from XAML text, such as `Margin="4,8"`.
//!
//! Numbers are separated by a comma, by whitespace, or by both: `"4,8"`, `"4 8"` and `"4, 8"` are
//! the same. Whitespace around the text is ignored. An empty value, such as two commas in a row or
//! a leading or trailing comma, is an error, and so are NaN and infinity.

use crate::geometry::{CornerRadius, Point, Rect, Size, Thickness, Transform};
use std::str::FromStr;

/// The error returned when text isn't a valid geometry value.
///
/// The message repeats the text, says what is wrong, and lists the forms the type accepts.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("invalid {type_name} '{text}': {problem}; expected {forms}")]
pub struct ParseGeometryError {
  /// The type being parsed, such as `Thickness`.
  pub type_name: &'static str,
  /// The text that failed to parse.
  pub text: String,
  /// What is wrong with it.
  pub problem: GeometryProblem,
  /// The accepted forms, as examples.
  pub forms: &'static str,
}

/// What is wrong with geometry text.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum GeometryProblem {
  /// A value between separators is missing.
  #[error("a value is empty")]
  Empty,
  /// A value isn't a number.
  #[error("'{0}' is not a number")]
  NotANumber(String),
  /// A value is NaN or infinite.
  #[error("'{0}' is not a finite number")]
  NotFinite(String),
  /// A value that must not be negative is.
  #[error("'{0}' is negative")]
  Negative(String),
  /// The text has a number of values the type doesn't accept.
  #[error("it has {0} values")]
  Count(usize),
  /// A transform function artic doesn't know, or text that isn't a function.
  #[error("'{0}' is not translate(x, y), rotate(degrees) or scale(x, y)")]
  UnknownFunction(String),
}

impl FromStr for Thickness {
  type Err = ParseGeometryError;

  /// Parses `"4"` (every side), `"4,8"` (left and right, then top and bottom) or `"1,2,3,4"`
  /// (left, top, right, bottom). Negative values are allowed.
  fn from_str(text: &str) -> Result<Self, Self::Err> {
    parse(
      text,
      "Thickness",
      r#""4", "4,8" or "1,2,3,4""#,
      Sign::Any,
      |numbers| match *numbers {
        [width] => Some(Thickness::uniform(width)),
        [horizontal, vertical] => Some(Thickness::symmetric(horizontal, vertical)),
        [left, top, right, bottom] => Some(Thickness::new(left, top, right, bottom)),
        _ => None,
      },
    )
  }
}

impl FromStr for CornerRadius {
  type Err = ParseGeometryError;

  /// Parses `"4"` (every corner) or `"1,2,3,4"` (top-left, top-right, bottom-right, bottom-left).
  /// Radii can't be negative.
  fn from_str(text: &str) -> Result<Self, Self::Err> {
    parse(
      text,
      "CornerRadius",
      r#""4" or "1,2,3,4""#,
      Sign::NotNegative,
      |numbers| match *numbers {
        [radius] => Some(CornerRadius::uniform(radius)),
        [top_left, top_right, bottom_right, bottom_left] => {
          Some(CornerRadius::new(top_left, top_right, bottom_right, bottom_left))
        }
        _ => None,
      },
    )
  }
}

impl FromStr for Point {
  type Err = ParseGeometryError;

  /// Parses `"x,y"`. Negative values are allowed.
  fn from_str(text: &str) -> Result<Self, Self::Err> {
    parse(text, "Point", r#""x,y""#, Sign::Any, |numbers| match *numbers {
      [x, y] => Some(Point::new(x, y)),
      _ => None,
    })
  }
}

impl FromStr for Size {
  type Err = ParseGeometryError;

  /// Parses `"width,height"`. Neither can be negative.
  fn from_str(text: &str) -> Result<Self, Self::Err> {
    parse(
      text,
      "Size",
      r#""width,height""#,
      Sign::NotNegative,
      |numbers| match *numbers {
        [width, height] => Some(Size::new(width, height)),
        _ => None,
      },
    )
  }
}

impl FromStr for Rect {
  type Err = ParseGeometryError;

  /// Parses `"x,y,width,height"`. The position can be negative; the width and height can't.
  fn from_str(text: &str) -> Result<Self, Self::Err> {
    let forms = r#""x,y,width,height""#;
    let rect = parse(text, "Rect", forms, Sign::Any, |numbers| match *numbers {
      [x, y, width, height] => Some(Rect::new(x, y, width, height)),
      _ => None,
    })?;
    for extent in [rect.width, rect.height] {
      if extent < 0.0 {
        return Err(error(text, "Rect", forms, GeometryProblem::Negative(extent.to_string())));
      }
    }
    Ok(rect)
  }
}

impl FromStr for Transform {
  type Err = ParseGeometryError;

  /// Parses CSS transform functions: `translate(x, y)` or `translate(x)`, `rotate(degrees)`,
  /// `scale(s)` or `scale(x, y)`, such as `"translate(10, 20) rotate(45)"`. As in CSS, whitespace
  /// between functions is optional, none may come between a name and its parenthesis, and the
  /// rightmost function applies to the element first.
  fn from_str(text: &str) -> Result<Self, Self::Err> {
    const FORMS: &str = r#""translate(10, 20)", "rotate(45)", "scale(2)" or several, such as "translate(10, 0) rotate(45)""#;
    let fail = |problem| error(text, "Transform", FORMS, problem);
    let mut rest = text.trim();
    if rest.is_empty() {
      return Err(fail(GeometryProblem::Empty));
    }
    let mut functions = Vec::new();
    while !rest.is_empty() {
      let unknown = || fail(GeometryProblem::UnknownFunction(rest.to_owned()));
      let open = rest.find('(').ok_or_else(unknown)?;
      let close = rest.find(')').filter(|close| *close > open).ok_or_else(unknown)?;
      let name = &rest[..open];
      let numbers = read_numbers(&rest[open + 1..close], Sign::Any).map_err(fail)?;
      let function = match (name, numbers.as_slice()) {
        ("translate", [x]) => Transform::translate(*x, 0.0),
        ("translate", [x, y]) => Transform::translate(*x, *y),
        ("rotate", [degrees]) => Transform::rotate(degrees.to_radians()),
        ("scale", [both]) => Transform::scale(*both, *both),
        ("scale", [x, y]) => Transform::scale(*x, *y),
        ("translate" | "rotate" | "scale", numbers) => return Err(fail(GeometryProblem::Count(numbers.len()))),
        (other, _) => return Err(fail(GeometryProblem::UnknownFunction(other.to_owned()))),
      };
      functions.push(function);
      rest = rest[close + 1..].trim_start();
    }
    let rightmost_first = functions.into_iter().rev().reduce(Transform::then);
    Ok(rightmost_first.expect("the text isn't empty, so it has a function"))
  }
}

/// Whether the values may be negative.
#[derive(Clone, Copy, PartialEq)]
enum Sign {
  Any,
  NotNegative,
}

/// Reads the numbers in `text` and builds the value with `build`, which returns `None` for a count
/// of numbers the type doesn't accept.
fn parse<T>(
  text: &str,
  type_name: &'static str,
  forms: &'static str,
  sign: Sign,
  build: impl FnOnce(&[f32]) -> Option<T>,
) -> Result<T, ParseGeometryError> {
  let numbers = read_numbers(text, sign).map_err(|problem| error(text, type_name, forms, problem))?;
  build(&numbers).ok_or_else(|| error(text, type_name, forms, GeometryProblem::Count(numbers.len())))
}

fn read_numbers(text: &str, sign: Sign) -> Result<Vec<f32>, GeometryProblem> {
  let mut numbers = Vec::new();
  for segment in text.split(',') {
    let mut parts = segment.split_whitespace().peekable();
    if parts.peek().is_none() {
      return Err(GeometryProblem::Empty);
    }
    for part in parts {
      let number: f32 = part.parse().map_err(|_| GeometryProblem::NotANumber(part.to_owned()))?;
      if !number.is_finite() {
        return Err(GeometryProblem::NotFinite(part.to_owned()));
      }
      if sign == Sign::NotNegative && number < 0.0 {
        return Err(GeometryProblem::Negative(part.to_owned()));
      }
      numbers.push(number);
    }
  }
  Ok(numbers)
}

fn error(text: &str, type_name: &'static str, forms: &'static str, problem: GeometryProblem) -> ParseGeometryError {
  ParseGeometryError {
    type_name,
    text: text.to_owned(),
    problem,
    forms,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;

  /// Returns the problem `text` has as a `T`, panicking if it parses.
  fn problem<T: FromStr<Err = ParseGeometryError> + std::fmt::Debug>(text: &str) -> GeometryProblem {
    text.parse::<T>().expect_err("the text is invalid").problem
  }

  // Every Thickness form parses, with commas, spaces or both.
  #[test]
  fn thickness_accepts_one_two_or_four_values() {
    for text in ["4", " 4 "] {
      assert_eq!(text.parse(), Ok(Thickness::uniform(4.0)), "{text}");
    }
    for text in ["4,8", "4 8", "4, 8"] {
      assert_eq!(text.parse(), Ok(Thickness::symmetric(4.0, 8.0)), "{text}");
    }
    for text in ["1,2,3,4", "1 2 3 4", "1, 2 3,4"] {
      assert_eq!(text.parse(), Ok(Thickness::new(1.0, 2.0, 3.0, 4.0)), "{text}");
    }
    assert_eq!("-4,0.5".parse(), Ok(Thickness::symmetric(-4.0, 0.5)));
  }

  // Every CornerRadius form parses, with commas or spaces.
  #[test]
  fn corner_radius_accepts_one_or_four_values() {
    assert_eq!("4".parse(), Ok(CornerRadius::uniform(4.0)));
    for text in ["1,2,3,4", "1 2 3 4"] {
      assert_eq!(text.parse(), Ok(CornerRadius::new(1.0, 2.0, 3.0, 4.0)), "{text}");
    }
  }

  // Point, Size and Rect parse their single form.
  #[test]
  fn point_size_and_rect_parse() {
    assert_eq!("-1.5, 2".parse(), Ok(Point::new(-1.5, 2.0)));
    assert_eq!("30 40".parse(), Ok(Size::new(30.0, 40.0)));
    assert_eq!("-1,2,30,40".parse(), Ok(Rect::new(-1.0, 2.0, 30.0, 40.0)));
  }

  // Empty text and empty values between separators are rejected.
  #[test]
  fn empty_values_are_rejected() {
    for text in ["", "   ", "4,,8", ",4", "4,"] {
      assert_eq!(problem::<Thickness>(text), GeometryProblem::Empty, "{text:?}");
    }
  }

  // A count of values the type doesn't accept is rejected.
  #[test]
  fn wrong_counts_are_rejected() {
    assert_eq!(problem::<Thickness>("1,2,3"), GeometryProblem::Count(3));
    assert_eq!(problem::<CornerRadius>("1,2"), GeometryProblem::Count(2));
    assert_eq!(problem::<Point>("1"), GeometryProblem::Count(1));
  }

  // Non-numbers, NaN and infinity are rejected.
  #[test]
  fn non_numbers_and_non_finite_values_are_rejected() {
    assert_eq!(problem::<Thickness>("abc"), GeometryProblem::NotANumber("abc".to_owned()));
    assert_eq!(problem::<Thickness>("4,NaN"), GeometryProblem::NotFinite("NaN".to_owned()));
    assert_eq!(problem::<Size>("inf,4"), GeometryProblem::NotFinite("inf".to_owned()));
  }

  // Sizes, radii and a rectangle's extent can't be negative.
  #[test]
  fn negative_extents_are_rejected() {
    assert_eq!(problem::<Size>("-1,4"), GeometryProblem::Negative("-1".to_owned()));
    assert_eq!(problem::<CornerRadius>("-2"), GeometryProblem::Negative("-2".to_owned()));
    assert_eq!(problem::<Rect>("0,0,-3,4"), GeometryProblem::Negative("-3".to_owned()));
  }

  // The message repeats the text, says what is wrong, and lists the accepted forms.
  #[test]
  fn error_message_names_the_expected_forms() {
    let error = "1,2,3".parse::<Thickness>().expect_err("three values");

    assert_eq!(
      error.to_string(),
      r#"invalid Thickness '1,2,3': it has 3 values; expected "4", "4,8" or "1,2,3,4""#
    );
  }

  /// Where `transform` sends `point`, rounded to hundredths to absorb `f32` error.
  fn at(transform: Transform, x: f32, y: f32) -> (f32, f32) {
    let point = transform.apply(Point::new(x, y));
    ((point.x * 100.0).round() / 100.0, (point.y * 100.0).round() / 100.0)
  }

  // Each function parses, with one or two numbers where CSS allows.
  #[test]
  fn transform_functions_parse() {
    assert_eq!(at("translate(10, 20)".parse().unwrap(), 1.0, 1.0), (11.0, 21.0));
    assert_eq!(at("translate(10)".parse().unwrap(), 1.0, 1.0), (11.0, 1.0));
    assert_eq!(at("scale(2)".parse().unwrap(), 3.0, 4.0), (6.0, 8.0));
    assert_eq!(at("scale(2 3)".parse().unwrap(), 3.0, 4.0), (6.0, 12.0));
    assert_eq!(at("rotate(90)".parse().unwrap(), 1.0, 0.0), (0.0, 1.0));
  }

  // As in CSS, whitespace between functions is optional, but none may come between a function's
  // name and its parenthesis.
  #[test]
  fn transform_whitespace_follows_css() {
    let spaced: Transform = "translate(100, 0) rotate(90)".parse().unwrap();
    let packed: Transform = "translate(100, 0)rotate(90)".parse().unwrap();

    assert_eq!(at(packed, 1.0, 0.0), at(spaced, 1.0, 0.0));
    assert_eq!(
      "rotate (90)".parse::<Transform>().unwrap_err().problem,
      GeometryProblem::UnknownFunction("rotate ".to_owned())
    );
  }

  // As in CSS, the rightmost function applies to the element first: this rotates, then moves.
  #[test]
  fn transform_functions_compose_like_css() {
    let transform: Transform = "translate(100, 0) rotate(90)".parse().unwrap();

    assert_eq!(at(transform, 1.0, 0.0), (100.0, 1.0));
  }

  // Unknown functions, wrong argument counts, bad numbers and empty text are errors.
  #[test]
  fn invalid_transforms_are_errors() {
    let problem = |text: &str| text.parse::<Transform>().unwrap_err().problem;

    assert_eq!(problem("skew(10)"), GeometryProblem::UnknownFunction("skew".to_owned()));
    assert_eq!(problem("rotate(1, 2)"), GeometryProblem::Count(2));
    assert_eq!(problem("translate(a, 2)"), GeometryProblem::NotANumber("a".to_owned()));
    assert_eq!(
      problem("translate(1, 2"),
      GeometryProblem::UnknownFunction("translate(1, 2".to_owned())
    );
    assert_eq!(problem("  "), GeometryProblem::Empty);
  }
}
