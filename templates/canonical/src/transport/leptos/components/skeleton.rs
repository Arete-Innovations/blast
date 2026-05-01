use leptos::prelude::*;
use stylance::import_crate_style;

use crate::structs::leptos::SkeletonVariant;

import_crate_style!(style, "src/transport/leptos/components/skeleton.module.scss");

#[component]
pub fn Skeleton(#[prop(default = SkeletonVariant::Line)] variant: SkeletonVariant) -> impl IntoView {
    let variant_attr = variant.as_str();
    view! {
        <span class=style::skeleton data-variant=variant_attr aria-hidden="true"></span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skeleton_variants() {
        assert_eq!(SkeletonVariant::Line.as_str(), "line");
        assert_eq!(SkeletonVariant::Card.as_str(), "card");
        assert_eq!(SkeletonVariant::Avatar.as_str(), "avatar");
        assert_eq!(SkeletonVariant::Button.as_str(), "button");
    }
}
