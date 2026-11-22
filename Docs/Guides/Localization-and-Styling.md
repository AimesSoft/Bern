**English** · [简体中文](../zh-CN/Guides/Localization-and-Styling.md)

# Localization, Theming & `style!`

Bern treats localization, light/dark appearance, and motion as first-class build-time declarations rather than afterthoughts.

## 1. Inline localization (`tr!`)

Declare supported languages in `Bern.toml`:

```toml
[languages]
supported = ["zh-Hans", "en"]
default = "en"
strict = true   # default; rejects missing translations (B102) and bare display literals (B103)
```

Write translations right next to the control using `tr!`:

```rust
Text(content: tr! {
    "zh-Hans" => "待办（剩余 {n} 项）",
    "en" => "{n, plural, =0 {All done} one {1 task left} other {# tasks left}}";
    n: app.remaining
})
```

- **Compile-time checks**: omitting any required language produces `error[B102]`; writing a raw string literal directly in a user-visible property produces `error[B103]` in strict mode; mismatched interpolation names across languages fail to compile.
- **Formatting**: supports named interpolation, ICU plural branches (`plural`), date formatting (`{date, date, y-MM-dd}`), and duration formatting (see [Component Guide — Text & Formatting](../Reference/UI.md#text-content-and-formatting)).
- **Runtime switching**: drop `LanguagePicker` into a settings page or call `ui set-language --language en` via the CLI. Every backend updates visible strings live without resetting window drafts.

## 2. Automatic light/dark mode & brand color

Every Bern backend supports light mode, dark mode, and high-contrast appearance automatically with zero per-widget color setup:

```toml
[theme]
mode = "system"        # "system" (default), "light", or "dark"
brand_seed = "#2F6FE4" # optional brand accent color
```

- On **SwiftUI, WinUI 3, GTK 4, and Compose**, colors come from the OS theme; `brand_seed` maps to the platform accent.
- On **Web and GPUI**, `bern-theme` derives a complete light/dark/high-contrast palette from `brand_seed` with verified contrast ratios.
- Override appearance for a specific region (such as a dark video theater frame inside a light window) using `Stack(appearance: dark) { ... }`, or let users pick theme/accent at runtime via `WindowSet(appearance:, accent:)`.
- During development, test either mode without changing OS settings by launching with `BERN_APPEARANCE=dark` or `BERN_APPEARANCE=light`.

## 3. Customizing visuals & motion with `style!`

When an app needs brand-specific surfaces, spring physics, or hover/pressed transitions, declare a `style!` block in your UI crate:

```rust
style! {
    motion {
        snappy: spring(period: 0.24, damping: 0.85),
        jelly: spring(period: 0.42, damping: 0.45),
    }

    transition {
        pop_in: pop(from: 0.85, jelly),
    }

    Button.hero {
        background: accent,
        corner: 10,
        size: large,
        hover: scale(1.03, snappy),
        pressed: scale(0.96, snappy),
        settle: jelly,
    }
}
```

Apply named variants via `variant: hero` on components, and structural transitions via `appear:`, `disappear:`, `stagger:`, `reflow:`, and shared-element `hero:` / `hero_key:`.

- All backends respect OS reduced-motion settings automatically; for local testing, set `BERN_MOTION=reduce` (reduced motion) or `BERN_MOTION=slow` (10× slow motion on SwiftUI/GPUI).

## 4. Automatic compile-time skeletons

Bern projects loading skeletons directly from your real `ui!` tree at compile time — no duplicate skeleton layouts to maintain:

- Attach `skeleton: <bool>` (and optional `skeleton_count: <n>`) to containers such as `Grid`, `Column`, or `Row`, or let `Query` render skeletons automatically while loading.
- Built-in **120 ms anti-flicker delay** prevents flashes on fast queries, with staggered fade-in once real data arrives and full reduced-motion + accessibility support across all six backends.
- For bespoke placeholders, use the explicit `Skeleton(width:, height:, shape:, tone:)` component.
