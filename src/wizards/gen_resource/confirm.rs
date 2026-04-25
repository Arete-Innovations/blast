use crate::error::BlastResult;
use crate::io::traits::{Sink, SinkExt};
use crate::state::resource::ResourceState;
use dialoguer::{theme::ColorfulTheme, Confirm};
use ron::ser::PrettyConfig;

pub fn review_and_confirm(
    resource: &ResourceState,
    sink: &mut dyn Sink,
) -> BlastResult<bool> {
    let mut canonical = resource.clone();
    canonical.canonicalize();
    let preview = render_preview(&canonical)?;

    sink.info("=== resource state preview ===");
    for line in preview.lines() {
        sink.info(line.to_string());
    }
    sink.info("==============================");

    let theme = ColorfulTheme::default();
    let answer = Confirm::with_theme(&theme)
        .with_prompt("Write this to storage/blast/state/resources/<name>.ron?")
        .default(true)
        .interact()?;
    Ok(answer)
}

fn render_preview(resource: &ResourceState) -> BlastResult<String> {
    let config = PrettyConfig::new()
        .depth_limit(64)
        .indentor("  ".to_string())
        .struct_names(true);
    let body = ron::ser::to_string_pretty(resource, config)?;
    Ok(body)
}
