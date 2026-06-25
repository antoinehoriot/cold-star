# Contributing to Cold Star

Thanks for your interest in Cold Star! Bug fixes, balance tweaks, new content
(tuning, jobs, modules, techs, encounters), and UI polish are all welcome.

## Development setup

You need [Rust](https://rustup.rs/) (stable, edition 2024) and, for the web
frontend, the [Dioxus CLI](https://dioxuslabs.com/):

```sh
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli            # provides `dx`

dx serve -p app --platform web      # run with hot-reload
```

## Before you open a pull request

Run the same checks CI runs and make sure they pass:

```sh
cargo fmt --all -- --check
cargo clippy -p sim -p content -- -D warnings
cargo nextest run -p sim -p content   # or `cargo test -p sim -p content`
dx build -p app --platform web        # if you changed the UI
```

- Add or update tests for any engine change. New behavior should ship with a
  test; bug fixes should ship with a test that fails before your fix.
- Keep commits focused and write a clear message describing the *why*.
- Match the style of the surrounding code (comment density, naming, idioms).

## Architecture rules (please respect these)

The codebase is split into three crates with hard boundaries. Keeping them lets
the engine stay testable and the game deterministic.

- **`sim` is pure and deterministic.** No wall-clock, no file or network I/O, no
  unseeded randomness (`#![forbid(unsafe_code)]`). All game logic lives here.
- **`sim` must not depend on `content`.** Data flows the other way: `content`
  loads/validates RON and hands `sim` the registries it needs.
- **The determinism invariant is sacred:** `advance(n) == n × advance(1)`. If a
  change makes stepped and bulk advancement diverge, that's a real bug — there
  are tests guarding it. Use ordered collections (`BTreeMap`/`BTreeSet`) in any
  serialized or iterated state; never iterate a `HashMap` where order matters.
- **Saves are versioned.** Changing the serialized shape of game state bumps
  `SAVE_VERSION`; old versions are rejected, not migrated.
- **The UI is a read-only view.** `app` derives view-models from a `sim` snapshot
  and dispatches `Action`s back into the engine. UI control states (enabled/
  disabled, visibility) should be exact duals of the engine's own acceptance
  rules — the UI never offers an action the engine would reject.

## Adding content

Most balance and content lives in RON under `crates/content/data/` (tuning,
jobs, modules, techs, origins). The `content` crate validates it on load — if
you add a field, add a `validate` check and a loader test alongside it.

## Reporting bugs / proposing features

Use the GitHub issue templates. For bugs, include the steps to reproduce and
what you expected; for features, describe the player-facing problem first.

## Code of Conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md). By
participating you agree to uphold it.

## License

By contributing, you agree that your contributions are licensed under the
project's [MIT License](LICENSE).
