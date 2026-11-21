# substrate-core

Hexagonal **core** for substrate: pure domain types and port traits (`EnginePort`, `StorePort`, `TransportPort`, `RoutingPort`, `DispatchApi`, and the orchestration superset ports). No adapter dependencies.

```toml
substrate-core = { git = "https://github.com/KooshaPari/substrate", package = "substrate-core" }
```

Prefer the [`substrate`](../substrate) facade crate for new consumers.
