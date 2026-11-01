use super::FormatOptions;
use std::{io, path::Path};

impl FormatOptions {
    /// Read `max_line_length` from the path's EditorConfig hierarchy.
    /// Unset values and `off` use the default width; invalid values and
    /// configuration files that cannot be read return an error.
    /// Other EditorConfig properties do not affect the fixed style.
    pub fn for_path(path: &Path) -> Result<Self, String> {
        Ok(Self {
            line_width: line_width(path)?,
        })
    }
}

fn line_width(path: &Path) -> Result<usize, String> {
    let properties = editorconfig_properties(path)
        .map_err(|error| format!("cannot read .editorconfig: {error}"))?;
    let value = properties
        .get_raw_for_key("max_line_length")
        .filter_unset()
        .into_option();
    match value {
        None | Some("off") => Ok(100),
        Some(value) => value
            .parse::<usize>()
            .ok()
            .filter(|&width| width > 0)
            .ok_or_else(|| format!("invalid .editorconfig max_line_length {value:?}: expected a positive integer or off")),
    }
}

/// Resolves EditorConfig properties like `ec4rs::properties_of`, but reports
/// a configuration file that exists and cannot be opened instead of skipping it.
fn editorconfig_properties(path: &Path) -> Result<ec4rs::Properties, String> {
    use ec4rs::PropertiesSource;
    let path = std::env::current_dir()
        .map_err(|error| error.to_string())?
        .join(path);
    let mut files = Vec::new();
    for directory in path.ancestors().skip(1) {
        let config = directory.join(".editorconfig");
        match ec4rs::ConfigFile::open(&config) {
            Ok(file) => {
                let is_root = file.reader.is_root;
                files.push(file);
                if is_root {
                    break;
                }
            }
            Err(ec4rs::ParseError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("{}: {error}", config.display())),
        }
    }
    let mut properties = ec4rs::Properties::new();
    for file in files.iter_mut().rev() {
        file.apply_to(&mut properties, &path)
            .map_err(|error| error.to_string())?;
    }
    Ok(properties)
}
