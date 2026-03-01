pub mod custom;
pub mod overview;
pub mod patterns;
pub mod tagger;

use std::path::Path;

use crate::error::Result;
use crate::types::SecurityTagDefinition;

pub fn load_tag_definitions(
    custom_file: Option<&Path>,
    plugin_files: &[std::path::PathBuf],
) -> Result<Vec<SecurityTagDefinition>> {
    let mut defs = patterns::builtin_patterns();

    if let Some(path) = custom_file {
        let custom = custom::load_custom_patterns(path)?;
        match custom.mode {
            custom::MergeMode::Extend => defs.extend(custom.tags),
            custom::MergeMode::Replace => defs = custom.tags,
        }
    }

    for plugin_file in plugin_files {
        let plugin = custom::load_custom_patterns(plugin_file.as_path())?;
        match plugin.mode {
            custom::MergeMode::Extend => defs.extend(plugin.tags),
            custom::MergeMode::Replace => defs = plugin.tags,
        }
    }

    Ok(defs)
}
