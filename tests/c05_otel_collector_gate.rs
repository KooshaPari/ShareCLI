// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C05 OTel local collector gate — T-1130 (Wave19 gap remediation).
//!
//! Validates that the local OTel collector stack files exist, parse as
//! valid YAML, advertise the required ports, and contain all exporters
//! needed for local development traces.
//!
//! FR: FR-003
//! Pillar: C05 L45+ (observability)

use std::path::Path;

// ── helpers ───────────────────────────────────────────────────────────────────

fn repo_root() -> std::path::PathBuf {
    // tests/ runs from the workspace root; walk up to find Cargo.toml marker.
    let mut dir = std::env::current_dir().expect("cwd");
    loop {
        if dir.join("Cargo.toml").exists() && dir.join("docker-compose.otel.yml").exists() {
            return dir;
        }
        dir = dir.parent().expect("repo root must have Cargo.toml").to_path_buf();
    }
}

fn read_yaml(path: &Path) -> serde_yaml::Value {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    serde_yaml::from_str(&content)
        .unwrap_or_else(|e| panic!("invalid YAML in {}: {e}", path.display()))
}

// ── tests ────────────────────────────────────────────────────────────────────

/// docker-compose.otel.yml must exist at the repo root.
#[test]
fn c05_docker_compose_otel_exists() {
    let root = repo_root();
    let path = root.join("docker-compose.otel.yml");
    assert!(path.exists(), "docker-compose.otel.yml must exist at {}", path.display());
}

/// docker-compose.otel.yml must be valid YAML.
#[test]
fn c05_docker_compose_otel_valid_yaml() {
    let root = repo_root();
    let path = root.join("docker-compose.otel.yml");
    let doc = read_yaml(&path);

    // Must have a `services` top-level key.
    let services = doc.get("services").expect("docker-compose.otel.yml must have `services` key");
    assert!(services.is_mapping(), "`services` must be a mapping");
}

/// docker-compose.otel.yml must declare the otel-collector service.
#[test]
fn c05_compose_declares_otel_collector_service() {
    let root = repo_root();
    let path = root.join("docker-compose.otel.yml");
    let doc = read_yaml(&path);

    let services = doc.get("services").expect("services");
    assert!(
        services.get("otel-collector").is_some(),
        "docker-compose.otel.yml must declare an `otel-collector` service"
    );
}

/// docker-compose.otel.yml must declare the jaeger service.
#[test]
fn c05_compose_declares_jaeger_service() {
    let root = repo_root();
    let path = root.join("docker-compose.otel.yml");
    let doc = read_yaml(&path);

    let services = doc.get("services").expect("services");
    assert!(
        services.get("jaeger").is_some(),
        "docker-compose.otel.yml must declare a `jaeger` service"
    );
}

/// docker-compose.otel.yml must expose OTLP gRPC (4317), HTTP (4318),
/// Prometheus (8888), and Jaeger UI (16686).
#[test]
fn c05_compose_exposes_required_ports() {
    let root = repo_root();
    let path = root.join("docker-compose.otel.yml");
    let doc = read_yaml(&path);

    let collector = doc.get("services").unwrap().get("otel-collector").unwrap();
    let ports = collector.get("ports").expect("otel-collector must have ports");
    let port_strs: Vec<String> = ports
        .as_sequence()
        .expect("ports must be a sequence")
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_string())
        .collect();
    let joined = port_strs.join(" ");

    for required in &["4317:4317", "4318:4318", "8888:8888", "16686:16686"] {
        assert!(
            joined.contains(required),
            "otel-collector ports must include {required}; found: {joined}"
        );
    }
}

/// otel-collector-config.yaml must exist at the repo root.
#[test]
fn c05_otel_collector_config_exists() {
    let root = repo_root();
    let path = root.join("otel-collector-config.yaml");
    assert!(path.exists(), "otel-collector-config.yaml must exist at {}", path.display());
}

/// otel-collector-config.yaml must be valid YAML.
#[test]
fn c05_otel_collector_config_valid_yaml() {
    let root = repo_root();
    let path = root.join("otel-collector-config.yaml");
    let doc = read_yaml(&path);

    // Must have the three top-level sections the collector expects.
    for key in &["receivers", "exporters", "service"] {
        assert!(doc.get(*key).is_some(), "otel-collector-config.yaml must have a `{key}` section");
    }
}

/// Collector config must define an OTLP receiver with both gRPC and HTTP.
#[test]
fn c05_collector_config_has_otlp_receiver() {
    let root = repo_root();
    let path = root.join("otel-collector-config.yaml");
    let doc = read_yaml(&path);

    let receivers = doc.get("receivers").expect("receivers section");
    let otlp = receivers.get("otlp").expect("must have `otlp` receiver");
    let protocols = otlp.get("protocols").expect("otlp must have `protocols` key");
    assert!(
        protocols.get("grpc").is_some(),
        "OTLP receiver must define gRPC protocol under protocols"
    );
    assert!(
        protocols.get("http").is_some(),
        "OTLP receiver must define HTTP protocol under protocols"
    );
}

/// Collector config must have Jaeger exporter (traces → Jaeger).
#[test]
fn c05_collector_config_has_jaeger_exporter() {
    let root = repo_root();
    let path = root.join("otel-collector-config.yaml");
    let doc = read_yaml(&path);

    let exporters = doc.get("exporters").expect("exporters section");
    let has_jaeger = exporters
        .as_mapping()
        .unwrap()
        .keys()
        .any(|k| k.as_str().map(|s| s.contains("jaeger")).unwrap_or(false));
    assert!(has_jaeger, "collector config must include a Jaeger exporter");
}

/// Collector config must have Prometheus exporter (self-metrics).
#[test]
fn c05_collector_config_has_prometheus_exporter() {
    let root = repo_root();
    let path = root.join("otel-collector-config.yaml");
    let doc = read_yaml(&path);

    let exporters = doc.get("exporters").expect("exporters section");
    let has_prom = exporters
        .as_mapping()
        .unwrap()
        .keys()
        .any(|k| k.as_str().map(|s| s.contains("prometheus")).unwrap_or(false));
    assert!(has_prom, "collector config must include a Prometheus exporter");
}

/// Collector config must have Debug exporter (stdout, for dev).
#[test]
fn c05_collector_config_has_debug_exporter() {
    let root = repo_root();
    let path = root.join("otel-collector-config.yaml");
    let doc = read_yaml(&path);

    let exporters = doc.get("exporters").expect("exporters section");
    let has_debug = exporters.as_mapping().unwrap().keys().any(|k| k.as_str() == Some("debug"));
    assert!(has_debug, "collector config must include a `debug` exporter");
}

/// Collector config service must declare a traces pipeline.
#[test]
fn c05_collector_config_has_traces_pipeline() {
    let root = repo_root();
    let path = root.join("otel-collector-config.yaml");
    let doc = read_yaml(&path);

    let service = doc.get("service").expect("service section");
    let pipelines = service.get("pipelines").expect("service must have pipelines");
    assert!(pipelines.get("traces").is_some(), "service must declare a `traces` pipeline");
}
