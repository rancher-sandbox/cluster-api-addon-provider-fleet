NAME := "cluster-api-fleet-controller"
KUBE_VERSION := env_var_or_default('KUBE_VERSION', '1.26.3')
ORG := "ghcr.io/rancher-sandbox"
TAG := "dev"
HOME_DIR := env_var('HOME')
YQ_VERSION := "v4.43.1"
YQ_BIN := "_out/yq"
KUSTOMIZE_VERSION := "v5.4.1"
KUSTOMIZE_BIN := "_out/kustomize"
ARCH := if arch() == "aarch64" { "arm64"} else { "amd64" }
DIST := os()

[private]
default:
    @just --list --unsorted --color=always

# Generates stuff
generate:
    just generate-crds

# generates files for CRDS
generate-crds: _create-out-dir _install-kopium _download-yq
    just _generate-kopium-url {{home_directory()}}/.cargo/bin/kopium "https://raw.githubusercontent.com/kubernetes-sigs/cluster-api/main/config/crd/bases/cluster.x-k8s.io_clusters.yaml" "src/api/capi_cluster.rs" ""
    just _generate-kopium-url {{home_directory()}}/.cargo/bin/kopium "https://raw.githubusercontent.com/rancher/fleet/main/charts/fleet-crd/templates/crds.yaml" "src/api/fleet_cluster.rs" "select(.spec.names.singular==\"cluster\")" "--no-condition"
    just _generate-kopium-url {{home_directory()}}/.cargo/bin/kopium "https://raw.githubusercontent.com/rancher/fleet/main/charts/fleet-crd/templates/crds.yaml" "src/api/fleet_clustergroup.rs" "select(.spec.names.singular==\"clustergroup\")" "--no-condition"

[private]
_generate-kopium-url kpath="" source="" dest="" yqexp="." condition="":
    curl -sSL {{source}} | {{YQ_BIN}} '{{yqexp}}' | {{kpath}} -D Default {{condition}} -f - > {{dest}}

# run with opentelemetry
run-telemetry:
    OPENTELEMETRY_ENDPOINT_URL=http://127.0.0.1:55680 RUST_LOG=info,kube=trace,controller=debug cargo run --features=telemetry

# run without opentelemetry
run:
    RUST_LOG=info,kube=debug,controller=debug cargo run

# format with nightly rustfmt
fmt:
    cargo +nightly fmt

# run unit tests
test-unit:
  cargo test

# compile for musl (for docker image)
compile features="":
  #!/usr/bin/env bash
  docker run --rm \
    -v cargo-cache:/root/.cargo \
    -v $PWD:/volume \
    -w /volume \
    -t clux/muslrust:stable \
    cargo build --release --features={{features}} --bin controller
  cp target/x86_64-unknown-linux-musl/release/controller _out/controller

[private]
_build features="":
  just compile {{features}}
  docker build -t {{ORG}}/{{NAME}}:{{TAG}} .

# docker build base
build-base: (_build "")
# docker build with telemetry
build-otel: (_build "telemetry")

load-base: build-base
    kind load docker-image {{ORG}}/{{NAME}}:{{TAG}} --name dev

# Start local dev environment
start-dev:
    rm -rf _out/ || true
    just update-helm-repos
    kind delete cluster --name dev || true
    kind create cluster --config --image=kindest/node:v{{KUBE_VERSION}} --config testdata/kind-config.yaml
    just install-fleet
    just install-capi
    kubectl wait pods --for=condition=Ready --timeout=300s --all --all-namespaces

# Stop the local dev environment
stop-dev:
    kind delete cluster --name dev || true

# Deploy CRS to dev cluster
deploy-crs:
    kubectl --context kind-dev apply -f testdata/crs.yaml

# Deploy child cluster using docker & kubead,
deploy-child-cluster:
    kubectl --context kind-dev apply -f testdata/cluster_docker_kcp.yaml

# Add and update helm repos used
update-helm-repos:
    #helm repo add gitea-charts https://dl.gitea.com/charts/
    helm repo add fleet https://rancher.github.io/fleet-helm-charts/
    #helm repo add jetstack https://charts.jetstack.io
    #helm repo add traefik https://traefik.github.io/charts
    #helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
    helm repo update

# Install fleet into the k8s cluster
install-fleet: _create-out-dir
    #!/usr/bin/env bash
    set -euxo pipefail
    kubectl config view -o json --raw | jq -r '.clusters[].cluster["certificate-authority-data"]' | base64 -d > _out/ca.pem
    API_SERVER_URL=`kubectl config view -o json --raw | jq -r '.clusters[] | select(.name=="kind-dev").cluster["server"]'`
    helm -n cattle-fleet-system install --create-namespace --wait fleet-crd fleet/fleet-crd
    helm install --create-namespace -n cattle-fleet-system --set apiServerURL=$API_SERVER_URL --set-file apiServerCA=_out/ca.pem fleet fleet/fleet --wait

# Install cluster api and any providers
install-capi:
    EXP_CLUSTER_RESOURCE_SET=true CLUSTER_TOPOLOGY=true clusterctl init -i docker

# Deploy will deploy the operator
deploy: _download-kustomize load-base
    {{KUSTOMIZE_BIN}} build config/default | kubectl apply -f -

undeploy: _download-kustomize
    {{KUSTOMIZE_BIN}} build config/default | kubectl delete --ignore-not-found=true -f -

release-manifests: _download-kustomize
    {{KUSTOMIZE_BIN}} build config/default > _out/addon-components.yaml

# Install kopium
[private]
_install-kopium:
    cargo install kopium

# Download kustomize
[private]
[linux]
[macos]
_download-kustomize:
    curl -sSL https://github.com/kubernetes-sigs/kustomize/releases/download/kustomize/{{KUSTOMIZE_VERSION}}/kustomize_{{KUSTOMIZE_VERSION}}_{{DIST}}_{{ARCH}}.tar.gz -o {{KUSTOMIZE_BIN}}.tar.gz
    tar -xzf {{KUSTOMIZE_BIN}}.tar.gz -C _out
    chmod +x {{KUSTOMIZE_BIN}}

# Download yq
[private]
[linux]
_download-yq:
    curl -sSL https://github.com/mikefarah/yq/releases/download/{{YQ_VERSION}}/yq_linux_{{ARCH}} -o {{YQ_BIN}}
    chmod +x {{YQ_BIN}}

[private]
[macos]
_download-yq:
    curl -sSL https://github.com/mikefarah/yq/releases/download/{{YQ_VERSION}}/yq_darwin_{{ARCH}} -o {{YQ_BIN}}
    chmod +x {{YQ_BIN}}

[private]
_create-out-dir:
    mkdir -p _out

