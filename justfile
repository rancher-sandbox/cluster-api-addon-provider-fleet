NAME := "cluster-api-fleet-controller"
KUBE_VERSION := env_var_or_default('KUBE_VERSION', '1.30.0')
ORG := "ghcr.io/rancher-sandbox"
TAG := "dev"
HOME_DIR := env_var('HOME')
YQ_VERSION := "v4.43.1"
CLUSTERCTL_VERSION := "v1.7.1"
OUT_DIR := "_out"
KUSTOMIZE_VERSION := "v5.4.1"
ARCH := if arch() == "aarch64" { "arm64"} else { "amd64" }
DIST := os()
REFRESH_BIN := env_var_or_default('REFRESH_BIN', '1')

export PATH := "_out:_out/bin:" + env_var('PATH')

[private]
default:
    @just --list --unsorted --color=always

# Generates stuff
generate features="":
    just generate-addon-crds {{features}}
    just generate-crds

# generates files for CRDS
generate-crds: _create-out-dir _install-kopium _download-yq
    just _generate-kopium-url kopium "https://raw.githubusercontent.com/kubernetes-sigs/cluster-api/main/config/crd/bases/cluster.x-k8s.io_clusters.yaml" "src/api/capi_cluster.rs" ""
    just _generate-kopium-url kopium "https://raw.githubusercontent.com/kubernetes-sigs/cluster-api/main/config/crd/bases/cluster.x-k8s.io_clusterclasses.yaml" "src/api/capi_clusterclass.rs" ""
    just _generate-kopium-url kopium "https://raw.githubusercontent.com/rancher/fleet/main/charts/fleet-crd/templates/crds.yaml" "src/api/fleet_cluster.rs" "select(.spec.names.singular==\"cluster\")" "--no-condition"
    just _generate-kopium-url kopium "https://raw.githubusercontent.com/rancher/fleet/main/charts/fleet-crd/templates/crds.yaml" "src/api/fleet_clustergroup.rs" "select(.spec.names.singular==\"clustergroup\")" "--no-condition"
    just _generate-kopium-url kopium "https://raw.githubusercontent.com/rancher/fleet/main/charts/fleet-crd/templates/crds.yaml" "src/api/fleet_cluster_registration_token.rs" "select(.spec.names.singular==\"clusterregistrationtoken\")" ""

[private]
_generate-kopium-url kpath="" source="" dest="" yqexp="." condition="":
    curl -sSL {{source}} | yq '{{yqexp}}' | {{kpath}} -D Default {{condition}} -A -d -f - > {{dest}}

generate-addon-crds features="":
    cargo run --features={{features}} --bin crdgen > config/crds/fleet-addon-config.yaml

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
compile features="":  _create-out-dir
  #!/usr/bin/env bash
  docker run --rm \
    -v cargo-cache:/root/.cargo \
    -v $PWD:/volume \
    -w /volume \
    -t clux/muslrust:stable \
    cargo build --release --features={{features}} --bin controller
  cp target/{{arch()}}-unknown-linux-musl/release/controller {{OUT_DIR}}/controller

[private]
_build features="":
  just compile {{features}}
  docker build -t {{ORG}}/{{NAME}}:{{TAG}} .

# docker build base
build-base: (_build "")

# docker build base with agent initiated
build-agent-initiated: (_build "agent-initiated")

# docker build with telemetry
build-otel: (_build "telemetry")

# Push the docker images
docker-push:
    docker push {{ORG}}/{{NAME}}:{{TAG}}

load-base features="":
    just _build {{features}}
    kind load docker-image {{ORG}}/{{NAME}}:{{TAG}} --name dev

# Start local dev environment
start-dev: _cleanup-out-dir _create-out-dir
    just update-helm-repos
    kind delete cluster --name dev || true
    kind create cluster --config --image=kindest/node:v{{KUBE_VERSION}} --config testdata/kind-config.yaml
    just install-fleet
    just install-capi
    kubectl wait pods --for=condition=Ready --timeout=150s --all --all-namespaces

# Stop the local dev environment
stop-dev:
    kind delete cluster --name dev || true

# Deploy CRS to dev cluster
deploy-cni:
    kubectl --context kind-dev apply -f testdata/cni.yaml

# Deploy an example app bundle to the cluster
deploy-app:
    kubectl --context kind-dev apply -f testdata/bundle.yaml

# Deploy child cluster using docker & kubeadm
deploy-child-cluster:
    kubectl --context kind-dev apply -f testdata/config.yaml
    kubectl --context kind-dev apply -f testdata/cluster_docker_kcp.yaml

# Deploy child cluster-call based cluster using docker & kubeadm
deploy-child-cluster-class:
    kind delete cluster --name capi-quickstart || true
    kubectl --context kind-dev apply -f testdata/config.yaml
    kubectl --context kind-dev apply -f testdata/capi-quickstart.yaml

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
    kubectl config view -o json --raw | jq -r '.clusters[].cluster["certificate-authority-data"]' | base64 -d > {{OUT_DIR}}/ca.pem
    API_SERVER_URL=`kubectl get nodes -o json | jq -r '.items[0].status.addresses[] | select(.type=="InternalIP").address'`
    API_SERVER_URL=https://${API_SERVER_URL}:6443
    helm -n cattle-fleet-system install --version v0.10.1-rc.1 --create-namespace --wait fleet-crd fleet/fleet-crd
    helm install --version v0.10.1-rc.1 --create-namespace -n cattle-fleet-system --set bootstrap.enabled=false --set apiServerURL=$API_SERVER_URL --set-file apiServerCA={{OUT_DIR}}/ca.pem fleet fleet/fleet --wait

# Install cluster api and any providers
install-capi: _download-clusterctl
    EXP_CLUSTER_RESOURCE_SET=true CLUSTER_TOPOLOGY=true clusterctl init -i docker

# Deploy will deploy the operator
deploy features="": _download-kustomize
    just generate {{features}}
    just load-base {{features}}
    kustomize build config/default | kubectl apply -f -

undeploy: _download-kustomize
    kustomize build config/default | kubectl delete --ignore-not-found=true -f -

release-manifests: _create-out-dir _download-kustomize
    kustomize build config/default > {{OUT_DIR}}/addon-components.yaml

# Full e2e test of importing cluster in fleet
test-import: start-dev deploy deploy-child-cluster deploy-cni deploy-app && collect-test-import
    kubectl wait pods --for=condition=Ready --timeout=150s --all --all-namespaces
    kubectl wait cluster --timeout=500s --for=condition=ControlPlaneReady=true docker-demo
    kubectl wait clusters.fleet.cattle.io --timeout=300s --for=condition=Ready=true docker-demo

collect-test-import:
    -just collect-artifacts dev
    -just collect-artifacts docker-demo

# Full e2e test of importing cluster in fleet
test-cluster-class-import: start-dev deploy deploy-child-cluster-class deploy-cni deploy-app _test-import-all && collect-test-cluster-class-import

collect-test-cluster-class-import:
    -just collect-artifacts dev
    -just collect-artifacts capi-quickstart

# Test e2e with agent initiated connection procedure
test-cluster-class-import-agent-initated: start-dev && collect-test-cluster-class-import
    just deploy "agent-initiated"
    just deploy-child-cluster-class
    just deploy-cni
    just deploy-app
    just _test-import-all

collect-artifacts cluster:
    kind get kubeconfig --name {{cluster}} > {{OUT_DIR}}/kubeconfig
    just crust-gather collect -f {{OUT_DIR}}/gather/{{cluster}} -k {{OUT_DIR}}/kubeconfig

# Full e2e test of importing cluster and ClusterClass in fleet
[private]
_test-import-all:
    kubectl wait pods --for=condition=Ready --timeout=150s --all --all-namespaces
    kubectl wait clustergroups.fleet.cattle.io --timeout=150s --for=jsonpath='{.status.clusterCount}=1' quick-start
    kubectl wait clustergroups.fleet.cattle.io --timeout=300s --for=condition=Ready=true quick-start
    kubectl wait clusters.fleet.cattle.io --timeout=300s --for=condition=Ready=true capi-quickstart

# Install kopium
[private]
_install-kopium:
    #!/usr/bin/env bash
    set -euxo pipefail
    [ -z `which kopium` ] || [ {{REFRESH_BIN}} != "0" ] || exit 0
    cargo install --git https://github.com/kube-rs/kopium.git --root {{OUT_DIR}}

download-kustomize: _download-kustomize

# Download kustomize
[private]
[linux]
[macos]
_download-kustomize:
    #!/usr/bin/env bash
    set -euxo pipefail
    [ -z `which kustomize` ] || [ {{REFRESH_BIN}} != "0" ] || exit 0
    curl -sSL https://github.com/kubernetes-sigs/kustomize/releases/download/kustomize/{{KUSTOMIZE_VERSION}}/kustomize_{{KUSTOMIZE_VERSION}}_{{DIST}}_{{ARCH}}.tar.gz -o {{OUT_DIR}}/kustomize.tar.gz
    tar -xzf {{OUT_DIR}}/kustomize.tar.gz -C {{OUT_DIR}}
    chmod +x {{OUT_DIR}}/kustomize

[private]
[linux]
_download-clusterctl:
    #!/usr/bin/env bash
    set -euxo pipefail
    [ -z `which clusterctl` ] || [ {{REFRESH_BIN}} != "0" ] || exit 0
    curl -L https://github.com/kubernetes-sigs/cluster-api/releases/download/{{CLUSTERCTL_VERSION}}/clusterctl-linux-{{ARCH}} -o {{OUT_DIR}}/clusterctl
    chmod +x {{OUT_DIR}}/clusterctl

[private]
[macos]
_download-clusterctl:
    #!/usr/bin/env bash
    set -euxo pipefail
    [ -z `which clusterctl` ] || [ {{REFRESH_BIN}} != "0" ] || exit 0
    curl -L https://github.com/kubernetes-sigs/cluster-api/releases/download/{{CLUSTERCTL_VERSION}}/clusterctl-darwin-{{ARCH}} -o {{OUT_DIR}}/clusterctl
    chmod +x {{OUT_DIR}}/clusterctl

# Download yq
[private]
[linux]
_download-yq:
    #!/usr/bin/env bash
    set -euxo pipefail
    [ -z `which yq` ] || [ {{REFRESH_BIN}} != "0" ] || exit 0
    curl -sSL https://github.com/mikefarah/yq/releases/download/{{YQ_VERSION}}/yq_linux_{{ARCH}} -o {{OUT_DIR}}/yq
    chmod +x {{OUT_DIR}}/yq

[private]
[macos]
_download-yq:
    #!/usr/bin/env bash
    set -euxo pipefail
    [ -z `which yq` ] || [ {{REFRESH_BIN}} != "0" ] || exit 0
    curl -sSL https://github.com/mikefarah/yq/releases/download/{{YQ_VERSION}}/yq_darwin_{{ARCH}} -o {{OUT_DIR}}/yq
    chmod +x {{OUT_DIR}}/yq

[private]
_create-out-dir:
    mkdir -p {{OUT_DIR}}

[private]
_cleanup-out-dir:
    rm -rf {{OUT_DIR}} || true

crust-gather *flags: _download-crust-gather
    crust-gather {{flags}}

[private]
_download-crust-gather: _create-out-dir
    #!/usr/bin/env bash
    set -euxo pipefail
    [ -z `which crust-gather` ] || [ {{REFRESH_BIN}} != "0" ] || exit 0
    curl -sSfL https://github.com/crust-gather/crust-gather/raw/main/install.sh | sh -s - -f -b {{OUT_DIR}}

