**English** · [简体中文](CONTRIBUTING.zh-CN.md)

# Contributing

## Checks before submitting

CI ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs the following checks on every pull request; run them locally before submitting:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
node --test crates/bern-svg/tests/*.test.mjs crates/bern-backend-web/tests/*.test.mjs
```

The GTK runtime `crates/bern-gtk` is not a workspace member and is checked separately. It needs the development packages for GTK 4.14, libadwaita 1.5 and WebKitGTK 6 (on Debian and Ubuntu, `libgtk-4-dev libadwaita-1-dev libwebkitgtk-6.0-dev`):

```sh
cd crates/bern-gtk
cargo fmt --check
cargo clippy --all-features --all-targets -- -D warnings
cargo test --all-features
```

With GTK 4 and libadwaita installed, the `bern-backend-gtk` tests also type-check the generated Linux app.

## Tests

- Tests must give the same result on any computer. Parts that need Xcode, the Android SDK or the Windows SDK check only what they can when the toolchain is missing, and must not fail because of it. Where only tool paths are needed, give the test an empty set of tool directories, as `crates/bern-cli/tests/svg_targets.rs` does.
- For generator changes, prefer tests that check the generated code; for targets whose generated output can be compiled on the local machine, the tests compile it as well.

## Runtime strings

Text that a runtime displays on its own is declared only once, in `FRAMEWORK_TEXTS` in [`crates/bern-target-api/src/texts.rs`](crates/bern-target-api/src/texts.rs), and must cover every framework language. Tests check that every language has a translation, and that every `bern.*` key used by the runtimes and generators is declared.

## Commit messages

Follow the style already used in the repository:

```text
<type>(<scope>): <one-sentence summary in lowercase English>

<body: why the change was made, what behavior changed, and, where needed, what did not change>
```

- `type` is `feat`, `fix`, `refactor`, `test`, `docs`, `ci` or `style`; `scope` is the part the change is in, for example `swiftui`, `gtk`, `compiler`, `targets`, and may be omitted when the change spans a lot.
- The summary says what the commit does, so that a reader understands it without opening the diff. Messages like `1` or `macos适配` say nothing about what changed.
- One commit does one thing; formatting, refactoring and behavior changes go in separate commits.

## Documentation

When components, properties or platform capabilities change, update the reference documents and acceptance records according to [the update rules in Docs/README.md](Docs/README.md#update-rules).

The documentation is in English and Simplified Chinese; a change to one page changes both languages in the same commit, and `cargo test -p bern --test docs` checks that they correspond.
