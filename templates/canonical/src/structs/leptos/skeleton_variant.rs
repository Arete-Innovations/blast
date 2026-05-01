#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SkeletonVariant {
    #[default]
    Line,
    Card,
    Avatar,
    Button,
}

impl SkeletonVariant {
    pub fn as_str(self) -> &'static str {
        match self {
            SkeletonVariant::Line => "line",
            SkeletonVariant::Card => "card",
            SkeletonVariant::Avatar => "avatar",
            SkeletonVariant::Button => "button",
        }
    }
}
