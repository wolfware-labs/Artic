use artic_core::property::Property;
use artic_element::{Child, ElementImpl};
use artic_layout::geometry::{CornerRadius, Rect, Size, Thickness};
use artic_layout::{ArrangeCx, MeasureCx};
use artic_macros::element;
use artic_render::{BoxShadow, Brush};

/// Draws a box around one child: a shadow, a background, an outline whose sides can differ in
/// width, rounded corners, and padding between the outline and the child, which it clips to the
/// inside of the outline.
#[element]
#[derive(Debug)]
#[xaml(namespace = "https://artic.rs/ui")]
pub struct Border {
  /// The child, drawn inside the padding and clipped to the inside of the outline.
  #[xaml(content)]
  pub child: Child,
  /// The space between the outline and the child. A side that isn't a finite number of 0 or more
  /// counts as 0.
  #[xaml(affects = layout)]
  pub padding: Property<Thickness>,
  /// The width of each side of the outline. A side that isn't a finite number of 0 or more counts
  /// as 0.
  #[xaml(affects = layout | render)]
  pub border_thickness: Property<Thickness>,
  /// The brush the box is filled with, under the outline, if any.
  #[xaml(affects = render)]
  pub background: Property<Option<Brush>>,
  /// The brush the outline is filled with, if any.
  #[xaml(affects = render)]
  pub border_brush: Property<Option<Brush>>,
  /// The radius of each corner of the outline.
  #[xaml(affects = render)]
  pub corner_radius: Property<CornerRadius>,
  /// The shadow the box casts, if any.
  #[xaml(affects = render)]
  pub box_shadow: Property<Option<BoxShadow>>,
}

impl Border {
  fn chrome(&self) -> (Thickness, Thickness) {
    (self.border_thickness.get().usable(), self.padding.get().usable())
  }
}

impl ElementImpl for Border {
  fn measure_override(&self, cx: &mut MeasureCx<'_>, available: Size) -> Size {
    let (thickness, padding) = self.chrome();
    let child = cx.children().next();
    let content = child.map_or(Size::ZERO, |child| cx.measure(child, available - thickness - padding));
    content + thickness + padding
  }

  fn arrange_override(&self, cx: &mut ArrangeCx<'_>, size: Size) {
    let (thickness, padding) = self.chrome();
    if let Some(child) = cx.children().next() {
      let inside = Rect::new(0.0, 0.0, size.width, size.height)
        .deflate(thickness)
        .deflate(padding);
      let origin = (inside.x.min(size.width), inside.y.min(size.height));
      cx.arrange(child, Rect::new(origin.0, origin.1, inside.width, inside.height));
    }
  }
}
