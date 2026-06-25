## What this changes

A short description of the change and the *why*.

## Checklist

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy -p sim -p content -- -D warnings` passes
- [ ] `cargo nextest run -p sim -p content` passes (tests added/updated for the change)
- [ ] `dx build -p app --platform web` succeeds (if the UI changed)
- [ ] Engine changes preserve determinism (`advance(n) == n × advance(1)`) and use ordered collections
- [ ] `SAVE_VERSION` bumped if the serialized state shape changed

## Notes for reviewers

Anything worth calling out — trade-offs, follow-ups, screenshots.
