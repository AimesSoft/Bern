//! Popup windows share engine/session state with their opener, not layout coordinates.

const RUNTIME: &str = include_str!("../templates/BernRuntime.kt");
const SCENE: &str = include_str!("../templates/BernScene.kt");
const FEATURES: &str = include_str!("../templates/BernFeatures.kt");
const VIEWS: &str = include_str!("../templates/BernViews.kt");

fn function<'a>(source: &'a str, name: &str) -> &'a str {
    let start = source.find(&format!("fun {name}(")).unwrap();
    let body = &source[start..];
    let end = body.find("\n}\n").unwrap() + 3;
    &body[..end]
}

#[test]
fn popup_owns_safe_area_instead_of_disabling_it() {
    let popup = function(RUNTIME, "BernPopupContent");
    for expected in [
        "remember { BernSafeArea() }",
        "LocalBernSafeArea provides safeArea",
        "LocalBernBarsPadding provides padding",
        "onPlaced { safeArea.window = it }.padding(padding)",
        "onPlaced { safeArea.safe = it }.consumeWindowInsets(padding)",
        "propagateMinConstraints = true",
    ] {
        assert!(popup.contains(expected), "missing {expected}");
    }
    assert!(!popup.contains("LocalBernSafeArea provides null"));
    // Do not create a new engine window or lose session identity.
    assert!(!popup.contains("LocalBernWindow provides"));
}

#[test]
fn popup_does_not_inherit_layout_registries() {
    let popup = function(RUNTIME, "BernPopupContent");
    for local in [
        "LocalBernScrollRoom",
        "LocalBernViewport",
        "LocalBernSceneRoom",
        "LocalBernShared",
        "LocalBernVisibility",
        "LocalBernFlex",
    ] {
        assert!(popup.contains(&format!("{local} provides null")), "{local}");
    }
    assert!(popup.contains("LocalBernPointer provides pointer"));
    assert!(popup.contains("LocalBernEditingRegions provides editing"));
    assert!(popup.contains("LocalBernEditingAncestors provides emptyList()"));
    assert!(popup.contains(".bernPointerWatch(pointer).bernEditingPresses(editing)"));
}

#[test]
fn every_custom_popup_surface_enters_its_own_scope() {
    for (source, names) in [
        (SCENE, &["BernModal", "BernSearchPalette"][..]),
        (
            FEATURES,
            &[
                "BernToolbarSearch",
                "BernInspector",
                "BernSplitButton",
                "BernPopoverButton",
            ][..],
        ),
        (VIEWS, &["BernTableRow", "BernMenu", "BernContextMenu"][..]),
    ] {
        for name in names {
            assert!(
                function(source, name).contains("BernPopupContent"),
                "{name}"
            );
        }
    }
    assert!(
        function(SCENE, "BernModal").contains(
            "if (covering) WindowInsets.systemBars.asPaddingValues() else PaddingValues()"
        )
    );
    assert!(
        function(SCENE, "BernSearchPalette").contains(
            "if (compact) WindowInsets.systemBars.asPaddingValues() else PaddingValues()"
        )
    );
}

#[test]
fn coordinate_conversion_rejects_other_roots_before_calling_compose() {
    let check = "if (!window.bernSharesLayout(safe) || !window.bernSharesLayout(coordinates)) return BernReach.None";
    assert!(RUNTIME.find(check).unwrap() < RUNTIME.find("window.localBoundingBoxOf(safe").unwrap());
    assert!(RUNTIME.contains(
        "isAttached && other.isAttached && findRootCoordinates() === other.findRootCoordinates()"
    ));
    assert!(RUNTIME.contains("room?.content?.takeIf { it.bernSharesLayout(coordinates) }"));
}
