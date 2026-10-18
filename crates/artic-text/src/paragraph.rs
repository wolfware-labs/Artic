//! A broken `parley` layout turned into a [`Paragraph`].
//!
//! The lines are placed here, not with `parley`'s `align`, which aligns against the break width
//! rather than the widest kept line. A left-to-right line starts at 0. A right-to-left line ends at
//! the paragraph's width, so it moves right by the width it lacks, and left by its trailing
//! whitespace, which `parley` lays at its left end.

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
  let mut runs = Vec::new();
  for (line, placed) in layout.lines().zip(&lines) {
    let shift = if layout.is_rtl() {
      width - placed.width - line.metrics().trailing_whitespace
    } else {
      0.0
    };
    runs.extend(line.items().filter_map(|item| match item {
      PositionedLayoutItem::GlyphRun(run) => Some(glyph_run(&run, shift, faces)),
      PositionedLayoutItem::InlineBox(_) => None,
    }));
  }
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

fn glyph_run(run: &parley::GlyphRun<'_, ()>, shift: f32, faces: &mut Faces) -> GlyphRun {
  let source = run.run();
  let synthesis = source.synthesis();
  let glyphs = run
    .positioned_glyphs()
    .map(|glyph| Glyph {
      id: glyph.id,
      x: glyph.x + shift,
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
