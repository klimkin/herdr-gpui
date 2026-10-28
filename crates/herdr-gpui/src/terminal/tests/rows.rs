use super::*;
use crate::terminal::rows::{bottom_aligned, whole_rows};

fn within(height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(3.), px(7.)), size(px(800.), px(height)))
}

#[test]
fn the_remainder_moves_above_the_first_row() {
    let outer = within(519.);
    let grid = bottom_aligned(outer, 20., 1.);
    assert_eq!(grid.bottom(), outer.bottom());
    assert_eq!(grid.size.height, px(500.));
    assert_eq!((grid.left(), grid.right()), (outer.left(), outer.right()));
}

#[test]
fn a_box_of_whole_rows_and_one_under_a_row_are_left_alone() {
    let exact = within(480.);
    assert_eq!(bottom_aligned(exact, 20., 1.), exact);
    let short = within(12.);
    assert_eq!(bottom_aligned(short, 20., 1.), short);
}

/// A fractional row height and display scale must never cost the daemon a
/// row: what the grid is sized to reads back as the rows it was sized for.
#[test]
fn the_rows_fitted_are_the_rows_asked_for() {
    for scale in [1., 1.25, 1.5, 2.] {
        for row in [18.57, 20., 21.4, 23.2, 30.] {
            for device in 400..1200 {
                let outer = within(device as f32 / scale);
                let grid = bottom_aligned(outer, row, scale);
                let rows = whole_rows(f32::from(outer.size.height), row);
                let context = format!("{device}px at {scale}x, rows of {row}");
                assert_eq!(grid.bottom(), outer.bottom(), "{context}");
                assert_eq!(
                    viewport(800., f32::from(grid.size.height), 10., row).rows,
                    rows as u16,
                    "{context}"
                );
                assert!(grid.top() >= outer.top() - px(1e-3), "{context}");
                assert!(grid.top() - outer.top() < px(row + 1. / scale), "{context}");
                let device_height = f32::from(grid.size.height) * scale;
                assert!(
                    (device_height - device_height.round()).abs() < 1e-3,
                    "{context}"
                );
            }
        }
    }
}
