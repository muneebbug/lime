//! The Compress manifest and the engine must agree on which formats to accept.
//!
//! Two lists that drift apart is exactly how Recolor ended up invisible for a
//! JPEG: the wheel filtered on one list and the engine used another, and the
//! tool silently vanished. The cross-check lives here because the dependency
//! runs from wheel-engines to wheel-core, so only this crate can see both.

use wheel_core::action::default_actions;
use wheel_engines::compress::{is_compressible, COMPRESSIBLE};

fn manifest_extents() -> Vec<String> {
    default_actions()
        .into_iter()
        .find(|a| a.id == "tool.compress")
        .expect("compress must be registered")
        .accepts
        .extensions
}

#[test]
fn the_manifest_and_the_engine_list_the_same_formats() {
    let mut from_manifest = manifest_extents();
    let mut from_engine: Vec<String> = COMPRESSIBLE.iter().map(|s| (*s).to_string()).collect();
    from_manifest.sort();
    from_engine.sort();

    assert_eq!(
        from_manifest, from_engine,
        "the wheel and the engine disagree about what Compress accepts"
    );
}

#[test]
fn every_advertised_extension_is_actually_accepted() {
    for ext in manifest_extents() {
        assert!(
            is_compressible(std::path::Path::new(&format!("a.{ext}"))),
            "the manifest offers {ext} but the engine refuses it"
        );
    }
}

#[test]
fn gif_is_never_offered() {
    // Re-encoding an animation risks flattening it to a single frame, which is
    // data loss rather than compression.
    assert!(!is_compressible(std::path::Path::new("a.gif")));
    assert!(!manifest_extents().contains(&"gif".to_string()));
}