//! Which rows of a box hold terminal cells.
//!
//! A box rarely stands a whole number of rows tall. The remainder is under one
//! row, and where it falls decides what the eye sees: below the last row, it
//! looks like a gap under the prompt; above the first, it is a thin margin
//! that reads as the window's own edge. [`bottom_rows`] puts it above.

use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, Style, Window, point, px, relative,
};

/// Fractions of a row a box may fall short of a whole row and still count as
/// holding it. A box sized to N rows comes back through float and device-pixel
/// rounding a hair under N rows tall, and must not lose one.
const SLACK: f32 = 1e-3;

/// How many whole rows of `cell_height` fit in `height`.
pub(crate) fn whole_rows(height: f32, cell_height: f32) -> f32 {
    (height / cell_height.max(1.) + SLACK).floor()
}

/// The bottom-aligned whole rows of `bounds`, on whole device pixels at
/// `scale`. A box under one row tall is left as it is.
pub(crate) fn bottom_aligned(
    bounds: Bounds<Pixels>,
    cell_height: f32,
    scale: f32,
) -> Bounds<Pixels> {
    let height = f32::from(bounds.size.height);
    let rows = whole_rows(height, cell_height);
    if rows < 1. {
        return bounds;
    }
    let grid = ((rows * cell_height.max(1.) * scale).ceil() / scale).min(height);
    Bounds::from_corners(
        point(bounds.left(), bounds.bottom() - px(grid)),
        bounds.bottom_right(),
    )
}

/// Lays `content` out in the whole rows at the bottom of the box it sits in,
/// so the sub-row remainder is above its first row. The cells' origin, hit
/// testing, painting, and the rows the daemon is asked for all read the
/// content's bounds, so they agree.
pub(crate) fn bottom_rows(cell_height: f32, content: impl IntoElement) -> BottomRows {
    BottomRows {
        cell_height,
        content: content.into_any_element(),
    }
}

pub(crate) struct BottomRows {
    cell_height: f32,
    content: AnyElement,
}

impl IntoElement for BottomRows {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for BottomRows {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        // The box the content is fitted into. The content is laid out on its
        // own once this box's size is known, so it is no child of this node.
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let grid = bottom_aligned(bounds, self.cell_height, window.scale_factor());
        self.content.prepaint_as_root(
            grid.origin,
            grid.size.map(AvailableSpace::Definite),
            window,
            cx,
        );
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.content.paint(window, cx);
    }
}
