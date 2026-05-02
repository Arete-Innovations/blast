#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonKind {
    #[default]
    Secondary,
    Primary,
    Danger,
    Ghost,
}
