use crate::error::{BlastError, BlastResult};
use crate::state::gen_level::GenLevel;
use crate::state::resource::ResourceState;
use dialoguer::{theme::ColorfulTheme, FuzzySelect};

pub fn collect_gen_level(resource: &mut ResourceState) -> BlastResult<()> {
    let theme = ColorfulTheme::default();
    let labels: Vec<String> = GenLevel::ALL.iter().map(|l| l.description().to_string()).collect();

    let default_idx = level_index(resource.gen_level);
    let idx = FuzzySelect::with_theme(&theme)
        .with_prompt("How far should codegen propagate for this resource?")
        .items(&labels)
        .default(default_idx)
        .interact()?;

    let chosen = match GenLevel::ALL.get(idx) {
        Some(level) => *level,
        None => {
            return Err(BlastError::Invalid(format!("gen_level FuzzySelect returned out-of-range index {idx}")));
        }
    };

    resource.gen_level = chosen;
    Ok(())
}

fn level_index(level: GenLevel) -> usize {
    let mut idx: usize = 0;
    for (i, candidate) in GenLevel::ALL.iter().enumerate() {
        if *candidate == level {
            idx = i;
            break;
        }
    }
    idx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_index_finds_default() {
        assert_eq!(level_index(GenLevel::default()), 4);
    }

    #[test]
    fn level_index_finds_struct() {
        assert_eq!(level_index(GenLevel::Struct), 0);
    }

    #[test]
    fn level_index_finds_pages() {
        assert_eq!(level_index(GenLevel::Pages), 6);
    }
}
