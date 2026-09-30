# Graveyard, trust, governance, provenance, finance

These modules apply the Lineage model to specific domains. They're part of the `lineage-rs` crate; `finance` is behind the default-on `finance` feature.

## Graveyard (`lineage::graveyard`)

When a lineage dies, the graveyard seals a **tombstone**: its identity, energy record, scars, and a pathology report, signed with HMAC-SHA256 and stored under `.lineage/graveyard/`. Tombstones can be listed and inspected, but not edited.

```rust
use lineage::Graveyard;

let _ = Graveyard::initialize();
let tombstones = Graveyard::list_all();
```

See [`docs/GRAVEYARD_GUIDE.md`](https://github.com/ecadelgrouplimited-dot/lineagers/blob/main/docs/GRAVEYARD_GUIDE.md) and `cargo run --example graveyard_inspector`.

## Trust (`lineage::trust`)

`TrustedActor` scores capability from behavior: violations lower trust and revoke capabilities, and revocations are permanent. See [`docs/TRUST_SYSTEM.md`](https://github.com/ecadelgrouplimited-dot/lineagers/blob/main/docs/TRUST_SYSTEM.md).

## Governance (`lineage::governance`)

Councils of members with finite voting energy vote on proposals, and every step is recorded in a governance ledger.

```rust
use lineage::{GovernanceCouncil, GovernanceConfig, ProposalRisk, VoteChoice};

let mut council = GovernanceCouncil::new(GovernanceConfig::default());
let member = council.add_member("Treasury".to_string(), 600);
let proposal = council.propose("Increase quorum".to_string(), ProposalRisk::Medium, 60);
council.vote(proposal.clone(), &member, VoteChoice::For)?;   // VoteReceipt { energy_cost: 25, … }
council.close(proposal)?;                                     // Passed
```

The `apps/governance-ops` console and `cargo run --example governance_ws_broadcast` show it live.

## Provenance (`lineage::provenance`)

Provenance is a hash-chained chain of custody for assets: creation, transfers, events, and sealing, each costing the vault energy.

```rust
use lineage::provenance::{MetadataHash, ProvenanceVault};

let mut vault = ProvenanceVault::new();
let asset = vault.create_asset("Vaulted Artifact".to_string(), MetadataHash::from_bytes(b"sha of the file"), "museum".to_string())?;
vault.transfer(&asset, "museum".to_string(), "lab".to_string(), 10)?;
vault.verify(&asset)?;   // VerifyReport { status: Valid, … }
```

See `cargo run --example provenance_chain_demo`.

## Finance (`lineage::finance`, feature `finance`)

Finance provides trading agents with finite capital, irreversible trades, financial scars from losses, evolutionary spawning, arenas, and market data from CoinMarketCap and CoinDesk. Learning agents are behind the `ml` feature. See [`docs/FINANCE_GETTING_STARTED.md`](https://github.com/ecadelgrouplimited-dot/lineagers/blob/main/docs/FINANCE_GETTING_STARTED.md) and `cargo run --example arena_with_live_market --release`.
