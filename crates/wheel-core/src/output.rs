use std::path::{Path, PathBuf};
use crate::settings::OutputPolicy;

/// Determine the output path for a converted file.
///
/// Rules:
/// - Never overwrite the source unless `overwrite_source` is explicitly true.
/// - Append `suffix` between stem and extension.
/// - On collision, append ` (N)` before the extension until a free slot is found.
pub fn resolve_output_path(
    source: &Path,
    target_ext: &str,
    suffix: &str,
    policy: &OutputPolicy,
    fixed_folder: Option<&Path>,
    overwrite_source: bool,
) -> PathBuf {
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let _source_ext = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let alongside_source = source
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    let parent = match policy {
        OutputPolicy::FixedFolder => fixed_folder
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| alongside_source.clone()),
        // Clipboard has no destination directory; the path is not used.
        OutputPolicy::Clipboard => alongside_source.clone(),
        OutputPolicy::NextToSource => alongside_source.clone(),
    };

    // Build base name
    let new_stem = format!("{}{}", stem, suffix);
    let new_name = format!("{}.{}", new_stem, target_ext);
    let candidate = parent.join(&new_name);

    // If the candidate equals the source and overwrite is forbidden, disambiguate
    if !overwrite_source && candidate == source {
        return disambiguate(&parent, &new_stem, target_ext);
    }

    // If the file already exists and overwrite is forbidden, disambiguate
    if !overwrite_source && candidate.exists() {
        return disambiguate(&parent, &new_stem, target_ext);
    }

    candidate
}

fn disambiguate(parent: &Path, stem: &str, ext: &str) -> PathBuf {
    for i in 1u32.. {
        let name = format!("{} ({}).{}", stem, i, ext);
        let path = parent.join(&name);
        if !path.exists() {
            return path;
        }
    }
    // Unreachable in practice
    parent.join(format!("{}.{}", stem, ext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_next_to_source_different_ext() {
        let src = Path::new("C:/Users/test/photo.png");
        let out = resolve_output_path(src, "webp", "", &OutputPolicy::NextToSource, None, false);
        // The output path is in the same directory with .webp extension
        assert_eq!(out.extension().unwrap().to_str().unwrap(), "webp");
        assert_eq!(out.file_stem().unwrap().to_str().unwrap(), "photo");
    }

    #[test]
    fn test_resolve_with_suffix() {
        let src = Path::new("C:/Users/test/photo.png");
        let out = resolve_output_path(src, "png", "-compressed", &OutputPolicy::NextToSource, None, false);
        assert_eq!(out.file_stem().unwrap().to_str().unwrap(), "photo-compressed");
    }

    #[test]
    fn test_fixed_folder_saves_into_the_chosen_folder() {
        let src = Path::new("C:/Users/test/pictures/photo.png");
        let chosen = Path::new("D:/Users/test/Desktop");

        let out = resolve_output_path(
            src,
            "webp",
            "",
            &OutputPolicy::FixedFolder,
            Some(chosen),
            false,
        );

        assert_eq!(out.parent().unwrap(), chosen);
        assert_eq!(out.file_name().unwrap().to_str().unwrap(), "photo.webp");
    }

    #[test]
    fn test_fixed_folder_without_a_path_falls_back_beside_the_source() {
        // A cleared folder must not produce a bare relative name.
        let src = Path::new("C:/Users/test/pictures/photo.png");

        let out = resolve_output_path(src, "webp", "", &OutputPolicy::FixedFolder, None, false);

        assert_eq!(out.parent().unwrap(), Path::new("C:/Users/test/pictures"));
    }

    #[test]
    fn test_next_to_source_ignores_any_folder_argument() {
        // Guard the contrast: NextToSource must not be swayed by a stale value.
        let src = Path::new("C:/Users/test/pictures/photo.png");

        let out = resolve_output_path(
            src,
            "webp",
            "",
            &OutputPolicy::NextToSource,
            Some(Path::new("D:/somewhere/else")),
            false,
        );

        assert_eq!(out.parent().unwrap(), Path::new("C:/Users/test/pictures"));
    }
}
