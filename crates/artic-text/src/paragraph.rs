//! A broken `parley` layout turned into a [`Paragraph`].

use crate::faces::Faces;
use artic_layout::geometry::Size;
use artic_layout::text::{Glyph, GlyphRun, Line, Paragraph, Synthesis};
use parley::{Layout, PositionedLayoutItem};

pub(crate) fn paragraph(layout: &Layout<()>, faces: &mut Faces) -> Paragraph {
  let lines = layout.lines().map(|line| line_of(line.metrics())).collect::<Vec<_>>();
  let width = lines.iter().map(|line| line.width).fold(0.0, f32::max);
  let height = lines.iter().map(|line| line.height).sum::<f32>();
  if height <= 0.0 {
    return Paragraph::default();
  }
  let runs = layout
    .lines()
    .flat_map(|line| line.items())
    .filter_map(|item| match item {
      PositionedLayoutItem::GlyphRun(run) => Some(glyph_run(&run, faces)),
      PositionedLayoutItem::InlineBox(_) => None,
    })
    .collect::<Vec<_>>();
  Paragraph::new(Size::new(width, height), lines, runs)
}

fn line_of(metrics: &parley::LineMetrics) -> Line {
  Line {
    top: metrics.block_min_coord,
    height: metrics.line_height,
    baseline: metrics.baseline,
    width: metrics.advance - metrics.trailing_whitespace,
  }
}

fn glyph_run(run: &parley::GlyphRun<'_, ()>, faces: &mut Faces) -> GlyphRun {
  let source = run.run();
  let synthesis = source.synthesis();
  let glyphs = run
    .positioned_glyphs()
    .map(|glyph| Glyph {
      id: glyph.id,
      x: glyph.x,
      y: glyph.y,
    })
    .collect::<Vec<_>>();
  GlyphRun::new(
    faces.font(source.font()),
    source.font_size(),
    source.normalized_coords(),
    Synthesis {
      embolden: synthesis.embolden(),
      skew: synthesis.skew().unwrap_or(0.0),
    },
    glyphs,
  )
}
