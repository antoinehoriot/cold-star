# Cold Star

A minimalist sci-fi incremental game about a small light in a very large dark —
and the cost of keeping it lit. Built in Rust, runs in the browser via
WebAssembly.

> You wake a dying reactor in a dead station. You survive by gathering, then by
> building, then by reaching out across the stars. The interface tells the story:
> it begins as a cold console you coax to life, and it changes as you do.

Inspired by the disclosure-driven storytelling of *A Dark Room* — almost no
exposition; the log, the research, the ruins, and the numbers carry the meaning.

---

## Status

Cold Star is **feature-complete** for its designed arc: a single run takes you
from scraping for warmth, through running a station and crew, out into a
procedurally generated galaxy, into away-team and ship encounters, to an earned
ending. The UI is a state-driven instrument — its warmth and color are real game
state, not decoration.

It is a finished small game, and an open invitation to extend: new content,
balance, encounters, and polish are all welcome (see
[CONTRIBUTING.md](CONTRIBUTING.md)).

## Play it locally

You need [Rust](https://rustup.rs/) (stable, edition 2024) and the
[Dioxus CLI](https://dioxuslabs.com/) for the web build:

```sh
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli            # provides the `dx` command

# run the game in your browser with hot-reload
dx serve -p app --platform web
```

Then open the URL it prints (usually <http://localhost:8080>).

To produce a static bundle instead:

```sh
dx build -p app --platform web      # output under target/dx/app/.../web/public
```

## How it's built

A small Rust workspace with a strict separation between the deterministic game
engine and its presentation:

| Crate | What it is |
|---|---|
| **`sim`** | The pure, deterministic game engine. No I/O, no randomness beyond a seeded RNG, `#![forbid(unsafe_code)]`. All game logic lives here. |
| **`content`** | Loads and validates game data (tuning, jobs, modules, techs, origins) from RON files into the registries `sim` consumes. `sim` never depends on `content`. |
| **`app`** | The [Dioxus](https://dioxuslabs.com/) web/WASM frontend. A thin, read-only view over `sim` state that dispatches player actions back into the engine. |

The core invariant the whole engine is built around:

```
advance(n) == n × advance(1)
```

Time is advanced in discrete ticks and the result is **identical** whether you
step one tick at a time or jump `n` ticks at once. This determinism makes the
game trivially testable, makes saves a plain snapshot, and is enforced by tests.
Saves are versioned and old formats are rejected rather than migrated.

```
crates/
  sim/        # pure deterministic engine (the game)
  content/    # RON data + validation
  app/        # Dioxus web UI
```

## Testing

```sh
cargo fmt --all -- --check
cargo clippy -p sim -p content -- -D warnings
cargo nextest run -p sim -p content   # or: cargo test -p sim -p content
```

The `sim` and `content` crates carry the test suite; `app` is verified by the
`dx build`. CI runs all of the above on every push and pull request.

## Contributing

Contributions are very welcome — bug fixes, balance, new content, encounters,
and UI polish especially. Please read [CONTRIBUTING.md](CONTRIBUTING.md) for the
dev setup, the architecture rules (the determinism invariant matters), and the
review checklist, and our [Code of Conduct](CODE_OF_CONDUCT.md).

## License

[MIT](LICENSE) © Antoine Horiot.
