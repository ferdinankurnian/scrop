use crate::style::{
    CANCEL_BUTTON_WIDTH, HANDLE_HIT_RADIUS, SELECT_BUTTON_HEIGHT, SELECT_BUTTON_WIDTH,
    TOOLBAR_CONTENT_GAP, TOOLBAR_GAP, TOOLBAR_HEIGHT, TOOLBAR_PADDING, TOOLBAR_WIDTH,
};

pub const MIN_SELECTION_SIZE: f64 = 24.0;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn from_points(a: Point, b: Point) -> Self {
        let x = a.x.min(b.x);
        let y = a.y.min(b.y);
        Self::new(x, y, (a.x - b.x).abs(), (a.y - b.y).abs())
    }

    pub fn right(self) -> f64 {
        self.x + self.width
    }

    pub fn bottom(self) -> f64 {
        self.y + self.height
    }

    pub fn center(self) -> Point {
        Point::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    pub fn contains(self, point: Point) -> bool {
        point.x >= self.x
            && point.x <= self.right()
            && point.y >= self.y
            && point.y <= self.bottom()
    }

    pub fn is_valid(self) -> bool {
        self.width >= MIN_SELECTION_SIZE && self.height >= MIN_SELECTION_SIZE
    }

    pub fn clamp_point(self, point: Point) -> Point {
        Point::new(
            point.x.clamp(self.x, self.right()),
            point.y.clamp(self.y, self.bottom()),
        )
    }

    pub fn translated(self, dx: f64, dy: f64, bounds: Rect) -> Self {
        let x = (self.x + dx).clamp(bounds.x, bounds.right() - self.width);
        let y = (self.y + dy).clamp(bounds.y, bounds.bottom() - self.height);
        Self::new(x, y, self.width, self.height)
    }

    pub fn resized(self, handle: Handle, pointer: Point, bounds: Rect) -> Self {
        let mut left = self.x;
        let mut top = self.y;
        let mut right = self.right();
        let mut bottom = self.bottom();

        if handle.moves_left() {
            left = pointer.x.clamp(bounds.x, right - MIN_SELECTION_SIZE);
        }
        if handle.moves_right() {
            right = pointer.x.clamp(left + MIN_SELECTION_SIZE, bounds.right());
        }
        if handle.moves_top() {
            top = pointer.y.clamp(bounds.y, bottom - MIN_SELECTION_SIZE);
        }
        if handle.moves_bottom() {
            bottom = pointer.y.clamp(top + MIN_SELECTION_SIZE, bounds.bottom());
        }

        Self::new(left, top, right - left, bottom - top)
    }

    pub fn relative_to(self, bounds: Rect) -> Self {
        Self::new(
            self.x - bounds.x,
            self.y - bounds.y,
            self.width,
            self.height,
        )
    }

    pub fn to_slurp_geometry(self) -> String {
        format!(
            "{},{} {}x{}",
            self.x.round() as i32,
            self.y.round() as i32,
            self.width.round() as i32,
            self.height.round() as i32
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    NorthWest,
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
}

impl Handle {
    pub const ALL: [Self; 8] = [
        Self::NorthWest,
        Self::North,
        Self::NorthEast,
        Self::East,
        Self::SouthEast,
        Self::South,
        Self::SouthWest,
        Self::West,
    ];

    pub fn center(self, rect: Rect) -> Point {
        let center = rect.center();
        match self {
            Self::NorthWest => Point::new(rect.x, rect.y),
            Self::North => Point::new(center.x, rect.y),
            Self::NorthEast => Point::new(rect.right(), rect.y),
            Self::East => Point::new(rect.right(), center.y),
            Self::SouthEast => Point::new(rect.right(), rect.bottom()),
            Self::South => Point::new(center.x, rect.bottom()),
            Self::SouthWest => Point::new(rect.x, rect.bottom()),
            Self::West => Point::new(rect.x, center.y),
        }
    }

    fn moves_left(self) -> bool {
        matches!(self, Self::NorthWest | Self::SouthWest | Self::West)
    }

    fn moves_right(self) -> bool {
        matches!(self, Self::NorthEast | Self::East | Self::SouthEast)
    }

    fn moves_top(self) -> bool {
        matches!(self, Self::NorthWest | Self::North | Self::NorthEast)
    }

    fn moves_bottom(self) -> bool {
        matches!(self, Self::SouthEast | Self::South | Self::SouthWest)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitTarget {
    Outside,
    Selection,
    Handle(Handle),
    CancelButton,
    SelectButton,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Interaction {
    Idle,
    Drawing {
        anchor: Point,
        output: usize,
    },
    Moving {
        pointer_origin: Point,
        selection_origin: Rect,
        output: usize,
    },
    Resizing {
        handle: Handle,
        selection_origin: Rect,
        output: usize,
    },
    PressingCancel,
    PressingSelect,
}

pub fn toolbar_rect(selection: Rect, bounds: Rect) -> Rect {
    let x = (selection.center().x - TOOLBAR_WIDTH / 2.0)
        .clamp(bounds.x, bounds.right() - TOOLBAR_WIDTH);
    let below = selection.bottom() + TOOLBAR_GAP;
    let y = if below + TOOLBAR_HEIGHT <= bounds.bottom() {
        below
    } else {
        (selection.y - TOOLBAR_GAP - TOOLBAR_HEIGHT).max(bounds.y)
    };
    Rect::new(x, y, TOOLBAR_WIDTH, TOOLBAR_HEIGHT)
}

pub fn cancel_button_rect(toolbar: Rect) -> Rect {
    let select = select_button_rect(toolbar);
    Rect::new(
        select.right() + TOOLBAR_CONTENT_GAP,
        toolbar.y + (toolbar.height - SELECT_BUTTON_HEIGHT) / 2.0,
        CANCEL_BUTTON_WIDTH,
        SELECT_BUTTON_HEIGHT,
    )
}

pub fn select_button_rect(toolbar: Rect) -> Rect {
    Rect::new(
        toolbar.x + TOOLBAR_PADDING,
        toolbar.y + (toolbar.height - SELECT_BUTTON_HEIGHT) / 2.0,
        SELECT_BUTTON_WIDTH,
        SELECT_BUTTON_HEIGHT,
    )
}

pub fn hit_test(point: Point, selection: Rect, bounds: Rect) -> HitTarget {
    let toolbar = toolbar_rect(selection, bounds);
    if select_button_rect(toolbar).contains(point) {
        return HitTarget::SelectButton;
    }
    if cancel_button_rect(toolbar).contains(point) {
        return HitTarget::CancelButton;
    }

    for handle in Handle::ALL {
        let center = handle.center(selection);
        if (point.x - center.x).abs() <= HANDLE_HIT_RADIUS
            && (point.y - center.y).abs() <= HANDLE_HIT_RADIUS
        {
            return HitTarget::Handle(handle);
        }
    }

    if selection.contains(point) {
        HitTarget::Selection
    } else {
        HitTarget::Outside
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUNDS: Rect = Rect::new(100.0, 50.0, 800.0, 600.0);

    #[test]
    fn normalizes_points() {
        assert_eq!(
            Rect::from_points(Point::new(90.0, 70.0), Point::new(10.0, 20.0)),
            Rect::new(10.0, 20.0, 80.0, 50.0)
        );
    }

    #[test]
    fn clamps_translation_to_output() {
        let rect = Rect::new(200.0, 150.0, 300.0, 200.0);
        assert_eq!(
            rect.translated(1000.0, -1000.0, BOUNDS),
            Rect::new(600.0, 50.0, 300.0, 200.0)
        );
    }

    #[test]
    fn resizes_from_corner_and_stays_in_output() {
        let rect = Rect::new(200.0, 150.0, 300.0, 200.0);
        assert_eq!(
            rect.resized(Handle::SouthEast, Point::new(1200.0, 900.0), BOUNDS),
            Rect::new(200.0, 150.0, 700.0, 500.0)
        );
    }

    #[test]
    fn prevents_resize_below_minimum() {
        let rect = Rect::new(200.0, 150.0, 300.0, 200.0);
        assert_eq!(
            rect.resized(Handle::West, Point::new(490.0, 0.0), BOUNDS),
            Rect::new(476.0, 150.0, MIN_SELECTION_SIZE, 200.0)
        );
    }

    #[test]
    fn toolbar_moves_above_selection_near_bottom_edge() {
        let selection = Rect::new(500.0, 600.0, 300.0, 40.0);
        assert!(toolbar_rect(selection, BOUNDS).bottom() <= selection.y);
    }

    #[test]
    fn toolbar_is_centered_on_selection() {
        let selection = Rect::new(300.0, 200.0, 400.0, 200.0);
        assert_eq!(
            toolbar_rect(selection, BOUNDS).center().x,
            selection.center().x
        );
    }

    #[test]
    fn hits_cancel_button() {
        let selection = Rect::new(200.0, 150.0, 300.0, 200.0);
        let cancel = cancel_button_rect(toolbar_rect(selection, BOUNDS));
        assert_eq!(
            hit_test(cancel.center(), selection, BOUNDS),
            HitTarget::CancelButton
        );
    }

    #[test]
    fn hits_resize_handle_before_selection() {
        let selection = Rect::new(200.0, 150.0, 300.0, 200.0);
        assert_eq!(
            hit_test(Point::new(200.0, 150.0), selection, BOUNDS),
            HitTarget::Handle(Handle::NorthWest)
        );
    }
}
