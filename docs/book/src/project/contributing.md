# Contributing

Lineage is MIT-licensed and developed at [github.com/ecadelgrouplimited-dot/lineagers](https://github.com/ecadelgrouplimited-dot/lineagers). Issues and pull requests are welcome.

## Build and test

The tests also build the examples' dev-dependencies, and one of them (`plotters`) needs the system fontconfig headers: `sudo apt-get install libfontconfig1-dev pkg-config` on Debian and Ubuntu, `brew install fontconfig` on macOS. Using the crate as a dependency needs none of this.


```sh
git clone https://github.com/ecadelgrouplimited-dot/lineagers
cd lineagers
cargo test                                   # default features
cargo test --no-default-features             # the core only
cargo test --features ml
cargo build --manifest-path apps/guard-server/Cargo.toml

# example apps: offline tests against a real guard server
cd apps/deepseek-payments-agent && python3 -m venv .venv && .venv/bin/pip install -r requirements.txt \
  && .venv/bin/python -m unittest discover -s tests
```

CI runs all of these on every push.

## Docs and website

- **Documentation:** `docs/book/`, built with [mdBook](https://rust-lang.github.io/mdBook/): `mdbook serve docs/book`.
- **Landing page and downloads:** `website/`.
- **Release artifacts:** `scripts/package-release.sh`.

## Principles

Changes should keep Lineage's guarantees intact: no rollback of history, no refunds of spent budget, no healing of scars, and no resurrection. If a feature needs one of those, it needs a new agent instead. See [`CONTRIBUTING.md`](https://github.com/ecadelgrouplimited-dot/lineagers/blob/main/CONTRIBUTING.md) for the full guidelines, and the [Security model](../concepts/security-model.md) for how to report vulnerabilities.
