# substrate

Rust SDK facade for the [substrate](https://github.com/KooshaPari/substrate) hexagonal dispatch spine.

Add one dependency instead of wiring `substrate-core`, `substrate-app`, and adapters yourself:

```toml
[dependencies]
substrate = { git = "https://github.com/KooshaPari/substrate", package = "substrate" }
# or, once published: substrate = "0.1"
```

## Usage

```rust
use substrate::{
    DispatchPlanner, EngineCandidate, EngineCapabilities, PlanRequest, SessionMode, TaskSpec,
    Task, TaskState, EnginePort, StorePort, TransportPort, DispatchApi,
};

// Plan without spawning
let spec = TaskSpec::new("implement feature X", "/my/repo");
let engines = vec![EngineCandidate {
    name: "forge".into(),
    capabilities: EngineCapabilities {
        supports_resume: true,
        supports_subagents: true,
        supports_mcp_import: false,
    },
}];
let plan = DispatchPlanner::plan(&PlanRequest {
    spec: &spec,
    engines: &engines,
    explicit_engine: None,
    session_mode: None,
    routing_engine: Some("forge"),
})?;
```

Optional features:

- `a2a` — A2A wire-schema types
- HTTP REST API — use the `driver-http` workspace crate (`substrate-http` binary)
- Adapter crates (`store-sqlite` with bundled SQLite, `engine-forge`) are separate workspace members — depend on them via git when needed

Default features: `app` + `spec` (planner + `TaskSpec`).
