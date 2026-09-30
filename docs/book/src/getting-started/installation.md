# Installation

Lineage is written in Rust and needs **Rust 1.89 or newer** to build from source. Prebuilt binaries need nothing.

## The Rust library

```sh
cargo add lineage-rs --no-default-features
```

The crate is named `lineage-rs`; you import it as `lineage`:

```rust
use lineage::guard::{Guard, Policy, ToolRule};
```

### Features

| Feature | Default | Adds |
|---|---|---|
| `finance` | on | The `finance` module (trading agents, arenas, market data). Pulls in `reqwest` and `tokio` |
| `cli` | on | The `lineage` binary |
| `ml` | off | `finance::ml`, learning agents (implies `finance`, pulls in `ndarray`) |

For the guard and the audit log alone, use `default-features = false`: the core then depends only on small, pure-Rust crates (`serde`, `sha2`, `ed25519-dalek`, `chrono`, `rand`, `hex`, `hmac`, `uuid`), with no network, async, or UI dependencies.

```toml
[dependencies]
lineage-rs = { version = "0.3", default-features = false }
```

## The CLI and guard server

### Install script (Linux x86-64)

```sh
curl -fsSL https://lineagrs.tech/install.sh | sh
```

The script:
- downloads the latest release from [lineagrs.tech/downloads](https://lineagrs.tech/downloads/);
- checks its SHA-256 checksum, and refuses to install on a mismatch;
- installs `lineage` and `guard-server` to `~/.local/bin`.

The binaries are statically linked, so they run on any Linux distribution.

| Variable | Default | |
|---|---|---|
| `LINEAGE_VERSION` | latest | Version to install |
| `LINEAGE_INSTALL_DIR` | `~/.local/bin` | Where to put the binaries |

### Manual download

Download `lineage-<version>-x86_64-linux.tar.gz` and `SHA256SUMS` from [the downloads page](https://lineagrs.tech/downloads/), then:

```sh
sha256sum --check --ignore-missing SHA256SUMS
tar -xzf lineage-*-x86_64-linux.tar.gz
```

### From source (any platform)

```sh
cargo install lineage-rs            # the lineage CLI

git clone https://github.com/ecadelgrouplimited-dot/lineagers
cd lineagers
cargo build --release --manifest-path apps/guard-server/Cargo.toml
# binary: apps/guard-server/target/release/guard-server
```

### Docker

```sh
git clone https://github.com/ecadelgrouplimited-dot/lineagers
cd lineagers
docker build -f apps/guard-server/Dockerfile -t lineage-guard .
docker run -p 9200:9200 -e GUARD_ADMIN_TOKEN=$(openssl rand -hex 32) -v guard-data:/data lineage-guard
```

The image runs as an unprivileged user and keeps its keys and logs in the `/data` volume.

## The Python client

The client is a single file with no dependencies, `lineage_guard.py`. Download it from [the downloads page](https://lineagrs.tech/downloads/), or copy it from `apps/guard-server/clients/python/` in the repository. It needs Python 3.8 or newer.

## Check the install

```sh
lineage --version
lineage audit keygen /tmp/test.key
```
