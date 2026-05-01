#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawerSide {
    Left,
    Right,
    Top,
    Bottom,
}

impl DrawerSide {
    pub fn as_str(self) -> &'static str {
        match self {
            DrawerSide::Left => "left",
            DrawerSide::Right => "right",
            DrawerSide::Top => "top",
            DrawerSide::Bottom => "bottom",
        }
    }
}
