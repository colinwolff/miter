//! Kubernetes, Compose and Litestream extraction, including secret redaction.

use std::path::Path;

use super::extract_yaml_symbols;
use crate::polyglot::SymbolKind;

const MANIFEST: &str = "\
apiVersion: v1
kind: Service
metadata:
  name: api-service
spec:
  ports:
    - port: 80
";

const DEPLOYMENT: &str = "\
apiVersion: apps/v1
kind: Deployment
metadata:
  name: api
spec:
  template:
    spec:
      containers:
        - name: api
          image: ghcr.io/acme/api:1.0
          env:
            - name: LOG_LEVEL
              value: debug
            - name: DATABASE_URL
              value: postgres://admin:secret@db/api
            - name: API_TOKEN
              valueFrom:
                secretKeyRef:
                  name: api-secrets
                  key: token
";

const COMPOSE: &str = "\
services:
  web:
    image: nginx:1.27
    environment:
      - name: APP_ENV
        value: production
      - name: SESSION_SECRET
        value: hunter2
  worker:
    build: ./worker
    environment:
      QUEUE: default
";

#[test]
fn a_kubernetes_resource_is_declared_by_kind_and_name() {
    let symbols = extract_yaml_symbols(Path::new("k8s/service.yaml"), MANIFEST);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, "Service/api-service");
    assert_eq!(symbols[0].kind, SymbolKind::K8sManifest);
    assert_eq!(symbols[0].line, 2);
    assert_eq!(symbols[0].signature, "K8S Service/api-service (api: v1)");
}

#[test]
fn a_workloads_environment_is_declared_and_its_literals_redacted() {
    let symbols = extract_yaml_symbols(Path::new("k8s/deploy.yaml"), DEPLOYMENT);
    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["Deployment/api", "LOG_LEVEL", "DATABASE_URL", "API_TOKEN"]
    );

    let by_name = |name: &str| symbols.iter().find(|s| s.name == name).expect("declared");
    assert_eq!(by_name("LOG_LEVEL").signature, "ENV LOG_LEVEL=debug");
    // A sequence entry's key sits on its own line, which is the line the declaration reports.
    assert_eq!(by_name("LOG_LEVEL").line, 12);
    assert_eq!(by_name("DATABASE_URL").line, 14);
    assert_eq!(by_name("API_TOKEN").line, 16);
    assert!(
        !by_name("DATABASE_URL").signature.contains("secret"),
        "{}",
        by_name("DATABASE_URL").signature
    );
    // A `secretKeyRef` names a secret rather than holding one, so the reference survives.
    assert_eq!(
        by_name("API_TOKEN").signature,
        "ENV API_TOKEN=secretKeyRef(api-secrets.token)"
    );
}

#[test]
fn compose_services_and_their_environment_are_declared() {
    let symbols = extract_yaml_symbols(Path::new("docker-compose.yml"), COMPOSE);
    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["web", "worker", "APP_ENV", "SESSION_SECRET", "QUEUE"]
    );

    let by_name = |name: &str| symbols.iter().find(|s| s.name == name).expect("declared");
    assert_eq!(by_name("web").signature, "SERVICE web (image: nginx:1.27)");
    assert_eq!(by_name("worker").signature, "SERVICE worker (build)");
    assert_eq!(by_name("APP_ENV").signature, "ENV APP_ENV=production");
    assert_eq!(by_name("APP_ENV").line, 5);
    assert_eq!(by_name("SESSION_SECRET").line, 7);
    assert_eq!(by_name("QUEUE").line, 12);
    assert_eq!(
        by_name("SESSION_SECRET").signature,
        "ENV SESSION_SECRET=[REDACTED_SECRET]"
    );
    assert_eq!(by_name("QUEUE").signature, "ENV QUEUE=default");
}

#[test]
fn a_config_map_and_a_secret_declare_their_keys() {
    let source = "apiVersion: v1
kind: ConfigMap
metadata:
  name: app-config
data:
  LOG_LEVEL: debug
  FEATURE_FLAGS: beta
---
apiVersion: v1
kind: Secret
metadata:
  name: app-secrets
stringData:
  API_TOKEN: ghp_1234567890abcdef
";
    let symbols = extract_yaml_symbols(Path::new("k8s/config.yaml"), source);
    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "ConfigMap/app-config",
            "LOG_LEVEL",
            "FEATURE_FLAGS",
            "Secret/app-secrets",
            "API_TOKEN"
        ]
    );
    let token = symbols
        .iter()
        .find(|s| s.name == "API_TOKEN")
        .expect("declared");
    assert_eq!(token.signature, "ENV API_TOKEN=[REDACTED_SECRET]");
    assert!(!token.signature.contains("ghp_"), "{}", token.signature);
    assert_eq!(
        symbols
            .iter()
            .find(|s| s.name == "LOG_LEVEL")
            .unwrap()
            .signature,
        "ENV LOG_LEVEL=debug"
    );
}

#[test]
fn litestream_replication_targets_are_declared() {
    let source =
        "dbs:\n  - path: /data/db.sqlite\n    replicas:\n      - url: s3://my-bucket/backups\n";
    let symbols = extract_yaml_symbols(Path::new("etc/litestream.yml"), source);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].kind, SymbolKind::K8sManifest);
    assert!(
        symbols[0]
            .signature
            .contains("LITESTREAM db=/data/db.sqlite")
    );
    assert!(
        symbols[0]
            .signature
            .contains("replica=s3://my-bucket/backups")
    );
}

#[test]
fn a_yaml_file_that_declares_nothing_yields_nothing() {
    let symbols = extract_yaml_symbols(
        Path::new("ci.yml"),
        "jobs:\n  build:\n    runs-on: ubuntu\n",
    );
    assert!(symbols.is_empty());
}
