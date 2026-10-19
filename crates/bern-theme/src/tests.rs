use super::*;

/// Seeds across hue, lightness and chroma, plus the extremes users pick.
fn seeds() -> Vec<Srgb> {
    let mut seeds = vec![
        DEFAULT_SEED,
        Srgb::BLACK,
        Srgb::WHITE,
        Srgb::new(0x80, 0x80, 0x80),
        Srgb::new(0xff, 0xff, 0x00),
        Srgb::new(0x00, 0xff, 0x00),
        Srgb::new(0xff, 0x00, 0xff),
        Srgb::new(0x00, 0xff, 0xff),
        Srgb::new(0xe4, 0x57, 0x2e),
        Srgb::new(0x6b, 0x7a, 0x8f),
    ];
    for hue in (0..360).step_by(15) {
        for l in [0.35, 0.55, 0.75, 0.9] {
            for c in [0.04, 0.12, 0.3] {
                seeds.push(Oklch::new(l, c, f64::from(hue)).to_srgb());
            }
        }
    }
    seeds
}

const MODES: [Mode; 2] = [Mode::Light, Mode::Dark];
const POLICIES: [ContrastPolicy; 2] = [ContrastPolicy::Standard, ContrastPolicy::More];

#[test]
fn every_declared_pairing_passes_for_every_seed_mode_and_policy() {
    let mut fallbacks = Vec::new();
    for seed in seeds() {
        for mode in MODES {
            for policy in POLICIES {
                let snapshot = generate(Some(seed), mode, policy);
                for check in &snapshot.checks {
                    assert!(
                        check.passed(),
                        "{seed} {mode:?} {policy:?}: {} on {} is {:.2}:1, needs {}:1",
                        check.foreground,
                        check.background,
                        check.ratio,
                        check.required
                    );
                }
                fallbacks.extend(
                    snapshot
                        .fallbacks
                        .iter()
                        .map(|reason| format!("{seed} {mode:?} {policy:?}: {reason}")),
                );
            }
        }
    }
    assert!(fallbacks.is_empty(), "{}", fallbacks.join("\n"));
}

#[test]
fn generation_is_deterministic() {
    for seed in [DEFAULT_SEED, Srgb::new(0xe4, 0x57, 0x2e)] {
        for mode in MODES {
            for policy in POLICIES {
                assert_eq!(
                    generate(Some(seed), mode, policy),
                    generate(Some(seed), mode, policy)
                );
            }
        }
    }
    assert_eq!(
        generate(None, Mode::Light, ContrastPolicy::Standard),
        generate(Some(DEFAULT_SEED), Mode::Light, ContrastPolicy::Standard)
    );
}

#[test]
fn surfaces_layer_the_web_way_and_dark_is_not_inverted_light() {
    let lum = |snapshot: &Snapshot, name: &str| snapshot.token(name).unwrap().luminance();
    let dark = generate(None, Mode::Dark, ContrastPolicy::Standard);
    let light = generate(None, Mode::Light, ContrastPolicy::Standard);
    // Dark: layers grow lighter as they rise.
    let order = [
        "surface.host",
        "surface.canvas",
        "surface.panel",
        "surface.raised",
    ];
    for pair in order.windows(2) {
        assert!(lum(&dark, pair[0]) < lum(&dark, pair[1]), "dark {pair:?}");
    }
    // Light: a white page and raised layers, a faint grey sidebar.
    assert_eq!(light.token("surface.canvas"), Some(Srgb::WHITE));
    assert_eq!(light.token("surface.raised"), Some(Srgb::WHITE));
    assert!(lum(&light, "surface.panel") < lum(&light, "surface.canvas"));
    assert!(lum(&light, "surface.host") < lum(&light, "surface.panel"));
    // Inverting the white page would give black; dark mode uses a dark
    // grey with room for text and raised layers.
    let canvas = dark.token("surface.canvas").unwrap();
    assert!(canvas.luminance() > 0.003 && canvas.luminance() < 0.05);
}

#[test]
fn selected_navigation_is_filled_with_the_accent() {
    let nav = RECIPES
        .iter()
        .find(|recipe| recipe.name == "nav_item")
        .unwrap();
    let selected = nav.state("selected").unwrap();
    assert!(selected.slots.contains(&("background", "accent")));
    assert!(selected.slots.contains(&("foreground", "on_accent")));
    assert!(selected.slots.iter().all(|(slot, _)| *slot != "indicator"));
    for mode in MODES {
        let snapshot = generate(None, mode, ContrastPolicy::Standard);
        let accent = snapshot.token("accent").unwrap();
        // The fill alone marks the item on the sidebar, and its label reads.
        assert!(contrast(accent, snapshot.token("surface.panel").unwrap()) >= 3.0);
        assert!(contrast(snapshot.token("on_accent").unwrap(), accent) >= 4.5);
        assert!(snapshot.token("selection.background").is_none());
    }
}

#[test]
fn brand_text_stays_as_vivid_as_reading_allows() {
    for mode in MODES {
        let snapshot = generate(None, mode, ContrastPolicy::Standard);
        let accent = snapshot.token("accent").unwrap().oklch();
        let text = snapshot.token("accent.text").unwrap().oklch();
        // Close to the accent, not a pale or muddy stand-in for it.
        assert!((text.l - accent.l).abs() < 0.12, "{mode:?}: {text:?}");
        assert!(text.c > 0.15, "{mode:?}: {text:?}");
    }
}

#[test]
fn media_progress_is_neutral_and_stands_out_from_its_track() {
    let magenta = Srgb::new(0xff, 0x00, 0xff);
    for mode in MODES {
        let snapshot = generate(Some(magenta), mode, ContrastPolicy::Standard);
        let played = snapshot.token("media.played").unwrap();
        let track = snapshot.token("media.track").unwrap();
        // Never the brand: a vivid seed leaves the played part grey.
        assert!(played.oklch().c < 0.03, "{mode:?}: {played}");
        assert!(contrast(played, track) >= 3.0, "{mode:?}");
        assert!(
            contrast(played, snapshot.token("surface.panel").unwrap()) >= 3.0,
            "{mode:?}"
        );
    }
}

#[test]
fn brand_roles_keep_the_seed_hue_and_status_ignores_it() {
    let orange = Srgb::new(0xe4, 0x57, 0x2e);
    for seed in [DEFAULT_SEED, orange] {
        let snapshot = generate(Some(seed), Mode::Light, ContrastPolicy::Standard);
        let hue_gap = |a: f64, b: f64| {
            let gap = (a - b).rem_euclid(360.0);
            gap.min(360.0 - gap)
        };
        let accent = snapshot.token("accent").unwrap().oklch();
        assert!(
            hue_gap(accent.h, seed.oklch().h) < 10.0,
            "{seed}: {accent:?}"
        );
    }
    let status = |seed: Srgb| {
        let snapshot = generate(Some(seed), Mode::Dark, ContrastPolicy::Standard);
        [
            "status.success",
            "status.warning",
            "status.error",
            "status.error.surface",
        ]
        .map(|name| snapshot.token(name).unwrap())
    };
    assert_eq!(status(DEFAULT_SEED), status(orange));
}

#[test]
fn more_contrast_raises_text_to_seven_to_one() {
    let snapshot = generate(None, Mode::Dark, ContrastPolicy::More);
    for name in ["text.primary", "text.secondary", "text.placeholder"] {
        let ratio = contrast(
            snapshot.token(name).unwrap(),
            snapshot.token("surface.raised").unwrap(),
        );
        assert!(ratio >= 7.0, "{name}: {ratio}");
    }
}

#[test]
fn recipes_reference_existing_tokens() {
    let snapshot = generate(None, Mode::Light, ContrastPolicy::Standard);
    for recipe in RECIPES {
        for state in recipe.states {
            for (slot, token) in state.slots {
                assert!(
                    *token == "transparent" || snapshot.token(token).is_some(),
                    "{}.{}.{slot} uses unknown token `{token}`",
                    recipe.name,
                    state.state
                );
            }
        }
        let interactive = !matches!(recipe.name, "window.chrome" | "dialog");
        assert_eq!(
            recipe.state("focus").is_some(),
            interactive,
            "{} focus state",
            recipe.name
        );
    }
}

#[test]
fn the_snapshot_carries_its_versions_and_inputs() {
    let snapshot = generate(Some(Srgb::new(1, 2, 3)), Mode::Dark, ContrastPolicy::More);
    assert_eq!(snapshot.schema_version, SCHEMA_VERSION);
    assert_eq!(snapshot.algorithm_version, ALGORITHM_VERSION);
    assert_eq!(snapshot.seed, Srgb::new(1, 2, 3));
    assert_eq!(
        (snapshot.mode, snapshot.contrast),
        (Mode::Dark, ContrastPolicy::More)
    );
    let names: Vec<_> = snapshot.tokens.iter().map(|token| token.name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(names, sorted, "tokens are sorted and unique");
}

#[test]
#[ignore = "prints the default palettes for review"]
fn print_default_palettes() {
    for mode in MODES {
        for policy in POLICIES {
            let snapshot = generate(None, mode, policy);
            println!("== {} {}", mode.name(), policy.name());
            for token in &snapshot.tokens {
                println!("{:28} {} {:3}", token.name, token.color, token.alpha);
            }
        }
    }
}

#[test]
fn accent_fills_carry_white_text_unless_the_accent_is_light() {
    for mode in MODES {
        // A red accent darkens a little to carry white text, as the
        // platforms' accent fills do.
        let red = generate(
            Some(Srgb::new(0xff, 0x2e, 0x55)),
            mode,
            ContrastPolicy::Standard,
        );
        assert_eq!(red.token("on_accent"), Some(Srgb::WHITE), "{mode:?}");
        assert!(contrast(Srgb::WHITE, red.token("accent").unwrap()) >= 4.5);
    }
    // In dark mode a bright green would have to turn dark to carry white:
    // it keeps its color and takes dark text.
    let green = generate(
        Some(Srgb::new(0x07, 0xff, 0x94)),
        Mode::Dark,
        ContrastPolicy::Standard,
    );
    assert_ne!(green.token("on_accent"), Some(Srgb::WHITE));
}
