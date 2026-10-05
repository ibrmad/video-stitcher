//! The arithmetic behind a camera's file list (`ui::file_list`): which row a
//! point is over, where a dragged row lands, and what the keys do.

/// The row under `y` points from the list's top, or `None` past the rows.
pub fn row_at(y: f64, row_height: f64, rows: usize) -> Option<usize> {
    if row_height <= 0.0 || y < 0.0 {
        return None;
    }
    let row = (y / row_height).floor() as usize;
    (row < rows).then_some(row)
}

/// Where a list `visible` points tall, scrolled to `scroll`, scrolls so the
/// row from `top` to `top + height` shows: where it is when the row shows
/// already, else just far enough.
pub fn scroll_to_show(top: f64, height: f64, scroll: f64, visible: f64) -> f64 {
    if top < scroll {
        top
    } else if top + height > scroll + visible {
        top + height - visible
    } else {
        scroll
    }
}

/// The gap between rows (0 before the first, `rows` after the last) nearest
/// `y`: where a dragged row would go.
pub fn gap_at(y: f64, row_height: f64, rows: usize) -> usize {
    if row_height <= 0.0 {
        return 0;
    }
    ((y / row_height).round().max(0.0) as usize).min(rows)
}

/// Where a row dragged from `from` ends up when dropped into `gap`, or
/// `None` when it stays where it is.
pub fn drop_index(from: usize, gap: usize) -> Option<usize> {
    if gap == from || gap == from + 1 {
        return None;
    }
    Some(if gap > from { gap - 1 } else { gap })
}

/// A key pressed on a focused list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListKey {
    /// Select the row above.
    Up,
    /// Select the row below.
    Down,
    /// Remove the selected row.
    Delete,
    /// Move the selected row up (Alt+↑).
    MoveUp,
    /// Move the selected row down (Alt+↓).
    MoveDown,
}

/// What a key asks of the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListCommand {
    /// Select this row.
    Select(usize),
    /// Remove this row.
    Remove(usize),
    /// Move the row at `from` to `to`.
    Move {
        /// Where it is.
        from: usize,
        /// Where it goes.
        to: usize,
    },
}

/// What `key` does with `selected` among `rows` rows.
pub fn key_command(key: ListKey, selected: Option<usize>, rows: usize) -> Option<ListCommand> {
    if rows == 0 {
        return None;
    }
    let last = rows - 1;
    match (key, selected) {
        (ListKey::Down, None) => Some(ListCommand::Select(0)),
        (ListKey::Up, None) => Some(ListCommand::Select(last)),
        (_, None) => None,
        (ListKey::Down, Some(s)) => (s < last).then(|| ListCommand::Select(s + 1)),
        (ListKey::Up, Some(s)) => (s > 0).then(|| ListCommand::Select(s - 1)),
        (ListKey::Delete, Some(s)) => Some(ListCommand::Remove(s)),
        (ListKey::MoveUp, Some(s)) => (s > 0).then(|| ListCommand::Move { from: s, to: s - 1 }),
        (ListKey::MoveDown, Some(s)) => {
            (s < last).then(|| ListCommand::Move { from: s, to: s + 1 })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_found_by_height() {
        assert_eq!(row_at(30.0, 24.0, 3), Some(1));
        assert_eq!(row_at(0.0, 24.0, 3), Some(0));
        assert_eq!(row_at(-1.0, 24.0, 3), None);
        assert_eq!(row_at(72.0, 24.0, 3), None);
        assert_eq!(row_at(10.0, 0.0, 3), None);
    }

    #[test]
    fn a_row_scrolls_just_into_view() {
        // Ten 24 pt rows show in 240 pt.
        assert_eq!(
            scroll_to_show(48.0, 24.0, 0.0, 240.0),
            0.0,
            "showing already"
        );
        assert_eq!(
            scroll_to_show(240.0, 24.0, 0.0, 240.0),
            24.0,
            "one below: one row"
        );
        assert_eq!(scroll_to_show(264.0, 24.0, 0.0, 240.0), 48.0);
        assert_eq!(
            scroll_to_show(24.0, 24.0, 48.0, 240.0),
            24.0,
            "above: to its top"
        );
    }

    #[test]
    fn a_drag_aims_at_the_nearest_gap() {
        assert_eq!(gap_at(5.0, 24.0, 3), 0);
        assert_eq!(gap_at(13.0, 24.0, 3), 1);
        assert_eq!(gap_at(40.0, 24.0, 3), 2);
        assert_eq!(gap_at(500.0, 24.0, 3), 3);
        assert_eq!(gap_at(-50.0, 24.0, 3), 0);
    }

    #[test]
    fn a_drop_beside_itself_moves_nothing() {
        assert_eq!(drop_index(0, 0), None);
        assert_eq!(drop_index(0, 1), None);
        assert_eq!(drop_index(0, 2), Some(1));
        assert_eq!(drop_index(2, 0), Some(0));
        assert_eq!(drop_index(1, 3), Some(2));
        assert_eq!(drop_index(2, 3), None);
    }

    #[test]
    fn keys_select_remove_and_move() {
        assert_eq!(
            key_command(ListKey::Down, None, 3),
            Some(ListCommand::Select(0))
        );
        assert_eq!(
            key_command(ListKey::Down, Some(1), 3),
            Some(ListCommand::Select(2))
        );
        assert_eq!(key_command(ListKey::Down, Some(2), 3), None);
        assert_eq!(key_command(ListKey::Up, Some(0), 3), None);
        assert_eq!(
            key_command(ListKey::Up, Some(2), 3),
            Some(ListCommand::Select(1))
        );
        assert_eq!(
            key_command(ListKey::Delete, Some(1), 3),
            Some(ListCommand::Remove(1))
        );
        assert_eq!(key_command(ListKey::Delete, None, 3), None);
        assert_eq!(
            key_command(ListKey::MoveUp, Some(1), 3),
            Some(ListCommand::Move { from: 1, to: 0 })
        );
        assert_eq!(key_command(ListKey::MoveDown, Some(2), 3), None);
        assert_eq!(key_command(ListKey::Down, None, 0), None);
    }
}
