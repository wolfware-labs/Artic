use artic_layout::text::{FontFamily, FontStyle, FontWeight, GenericFamily, Paragraph, TextEngine, TextLimits, TextStyle};
use artic_test_fonts::{BOLD, REGULAR};
use artic_text::Engine;
use pretty_assertions::assert_eq;
use std::sync::Arc;

const LINE: f32 = 19.84;

fn font(bytes: &'static [u8]) -> Arc<dyn AsRef<[u8]> + Send + Sync> {
  Arc::new(bytes)
}

fn engine() -> Engine {
  Engine::with_fonts([font(REGULAR), font(BOLD)])
}

fn style() -> TextStyle {
  TextStyle::new("Atkinson Hyperlegible".parse().unwrap(), 16.0)
}

fn shape(engine: &mut Engine, text: &str, style: &TextStyle) -> Paragraph {
  engine.paragraph(text, style, TextLimits::NONE)
}

fn assert_close(actual: f32, expected: f32) {
  assert!((actual - expected).abs() <= 0.5, "{actual} isn't within 0.5 of {expected}");
}

fn face(paragraph: &Paragraph) -> &[u8] {
  (**paragraph.runs()[0].font().bytes()).as_ref()
}

// The test font sets "Hello, world" at 16 px in one line of its own size.
#[test]
fn paragraph_size_with_test_font() {
  let paragraph = shape(&mut engine(), "Hello, world", &style());

  assert_close(paragraph.size().width, 81.49);
  assert_close(paragraph.size().height, LINE);
  assert_eq!(paragraph.lines().len(), 1);
}

// Empty text has no width and no glyphs, and the height of one line, as an empty WPF TextBlock.
#[test]
fn empty_text_has_one_line_height() {
  let paragraph = shape(&mut engine(), "", &style());

  assert_eq!(paragraph.size().width, 0.0);
  assert_close(paragraph.size().height, LINE);
  assert_eq!(paragraph.lines().len(), 1);
  assert!(paragraph.is_empty());
}

// A size that isn't finite or isn't above 0 gives an empty paragraph of size 0.
#[test]
fn bad_sizes_give_an_empty_paragraph() {
  for size in [0.0, -1.0, f32::NAN, f32::INFINITY] {
    let style = TextStyle { size, ..style() };

    assert_eq!(shape(&mut engine(), "Hello", &style), Paragraph::default());
  }
}

// An engine without fonts gives an empty paragraph of size 0.
#[test]
fn no_font_gives_an_empty_paragraph() {
  let paragraph = shape(&mut Engine::with_fonts([]), "Hello", &style());

  assert_eq!(paragraph, Paragraph::default());
}

// Bytes that aren't a font are skipped, and the first font that loads is the default family.
#[test]
fn unreadable_font_bytes_are_skipped() {
  let mut engine = Engine::with_fonts([font(b"not a font"), font(REGULAR)]);
  let nope = TextStyle::new("Nope".parse().unwrap(), 16.0);

  assert!(std::ptr::eq(face(&shape(&mut engine, "Hello", &nope)), REGULAR));
}

// A family that isn't installed gives way to the next one in the list.
#[test]
fn missing_family_falls_back() {
  let mut engine = engine();
  let listed = TextStyle::new("Nope, 'Atkinson Hyperlegible'".parse().unwrap(), 16.0);

  assert_eq!(shape(&mut engine, "Hello", &listed), shape(&mut engine, "Hello", &style()));
}

// With only its own fonts, an unknown name and every generic family land on the first font.
#[test]
fn with_fonts_falls_back_to_its_first_font() {
  let mut engine = engine();

  for family in ["Nope", "serif", "monospace", "system-ui"] {
    let style = TextStyle::new(family.parse().unwrap(), 16.0);

    assert!(std::ptr::eq(face(&shape(&mut engine, "Hello", &style)), REGULAR), "{family}");
  }
}

// Bold selects the family's bold face, with nothing synthesized.
#[test]
fn bold_selects_a_heavier_face() {
  let style = TextStyle {
    weight: FontWeight::BOLD,
    ..style()
  };
  let paragraph = shape(&mut engine(), "Hello", &style);

  assert!(std::ptr::eq(face(&paragraph), BOLD));
  assert!(!paragraph.runs()[0].synthesis().embolden);
}

// Bold asked of a family with only a regular face thickens the regular one.
#[test]
fn missing_bold_is_synthesized() {
  let style = TextStyle {
    weight: FontWeight::BOLD,
    ..style()
  };
  let paragraph = shape(&mut Engine::with_fonts([font(REGULAR)]), "Hello", &style);

  assert!(std::ptr::eq(face(&paragraph), REGULAR));
  assert!(paragraph.runs()[0].synthesis().embolden);
}

// Italic and oblique, asked of a family without an italic face, slant the upright one.
#[test]
fn missing_italic_is_synthesized() {
  for slant in [FontStyle::Italic, FontStyle::Oblique] {
    let style = TextStyle { style: slant, ..style() };
    let paragraph = shape(&mut engine(), "Hello", &style);

    assert!(paragraph.runs()[0].synthesis().skew > 0.0, "{slant:?}");
  }
}

// One face keeps one id across paragraphs, and another face has another id.
#[test]
fn same_font_keeps_its_id() {
  let mut engine = engine();
  let bold = TextStyle {
    weight: FontWeight::BOLD,
    ..style()
  };

  let first = shape(&mut engine, "Hello", &style());
  let second = shape(&mut engine, "world", &style());
  let heavy = shape(&mut engine, "Hello", &bold);

  assert_eq!(first.runs()[0].font().id(), second.runs()[0].font().id());
  assert_ne!(first.runs()[0].font().id(), heavy.runs()[0].font().id());
}

// A line height puts each line that far below the previous one.
#[test]
fn line_height_sets_line_spacing() {
  let style = TextStyle {
    line_height: Some(30.0),
    ..style()
  };
  let paragraph = shape(&mut engine(), "a\nb", &style);

  assert_eq!(paragraph.lines().len(), 2);
  assert_close(paragraph.lines()[1].top, 30.0);
  assert_close(paragraph.size().height, 60.0);
}

// A line height that isn't finite or isn't above 0 uses the font's own spacing.
#[test]
fn unusable_line_height_uses_the_font_spacing() {
  let mut engine = engine();
  let expected = shape(&mut engine, "a\nb", &style());

  for line_height in [0.0, -1.0, f32::NAN, f32::INFINITY] {
    let style = TextStyle {
      line_height: Some(line_height),
      ..style()
    };

    assert_eq!(shape(&mut engine, "a\nb", &style), expected, "{line_height}");
  }
}

// Text ending in a newline keeps the empty line after it, and its height counts.
#[test]
fn trailing_newline_keeps_its_empty_line() {
  let paragraph = shape(&mut engine(), "a\n", &style());

  assert_eq!(paragraph.lines().len(), 2);
  assert_close(paragraph.size().height, 2.0 * LINE);
}

// The system engine maps system-ui to an installed font. A machine with no fonts skips the test.
#[test]
fn system_engine_resolves_system_ui() {
  let paragraph = shape(&mut Engine::system(), "Hello", &TextStyle::new(FontFamily::default(), 16.0));

  if paragraph.is_empty() {
    eprintln!("skipped: no system fonts installed");
    return;
  }
  assert!(paragraph.size().width > 0.0);
  assert!(paragraph.size().height > 0.0);
}

// A system font that fontique's file cache dropped, after 200 paragraphs in another family,
// comes back with its first id. A machine with no fonts skips the test.
#[test]
fn system_font_keeps_its_id() {
  let mut engine = Engine::system();
  let ui = TextStyle::new(FontFamily::default(), 16.0);
  let mono = TextStyle::new(FontFamily::from(GenericFamily::Monospace), 16.0);

  let first = shape(&mut engine, "Hello", &ui);
  if first.is_empty() {
    eprintln!("skipped: no system fonts installed");
    return;
  }
  for _ in 0..200 {
    shape(&mut engine, "Hello", &mono);
  }
  let again = shape(&mut engine, "Hello", &ui);

  assert_eq!(again.runs()[0].font().id(), first.runs()[0].font().id());
}

// Debug shows the default family and how many faces the engine has drawn from.
#[test]
fn engine_debug_shows_default_family_and_faces() {
  let mut engine = Engine::with_fonts([font(REGULAR)]);
  shape(&mut engine, "Hello", &style());

  assert_eq!(
    format!("{engine:?}"),
    r#"Engine { default_family: Named("Atkinson Hyperlegible"), faces: 1 }"#
  );
}
