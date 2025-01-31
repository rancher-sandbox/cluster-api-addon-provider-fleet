# Cluster API Add-on Provider for Fleet

> NOTE: this is a work in progress. The project is looking for more contributors.

## What is Cluster API Add-on Provider for Fleet (CAAPF)?

Cluster API Add-on Provider for Fleet (CAAPF) is a Cluster API (CAPI) provider that provides integration with [Fleet](https://github.com/rancher/fleet) to enable the easy deployment of applications to a CAPI provisioned cluster.

It provides the following functionality:

- Addon provider automatically installs `Fleet` in your management cluster.
- The provider will register a newly provisioned CAPI cluster with `Fleet` so that applications can be automatically deployed to the created cluster via GitOps, `Bundle` or `HelmApp`.
- The provider will automatically create a [Fleet Cluster Group](https://fleet.rancher.io/cluster-group) for every [CAPI ClusterClass](https://cluster-api.sigs.k8s.io/tasks/experimental-features/cluster-class/). This enables you to deploy the same applications to all clusters created from the same ClusterClass.
- `CAPI` `Cluster`, `ControlPlane` resources are automatically added to the `Fleet` `Cluster` resource templates, allowing to perform per-cluster configuration templating for `Helm` based installations.

## Demo

[![asciicast](https://asciinema.org/a/659626.svg)](https://asciinema.org/a/659626)

## Calico CNI installation demo

[![asciicast](https://asciinema.org/a/700924.svg)](https://asciinema.org/a/700924)

## Getting started

You can refer to the provider documentation [here](./docs/book/src/.).

## Installation

You can install production instance of `CAAPF` in your cluster with [`CAPI Operator`](https://github.com/kubernetes-sigs/cluster-api-operator).

```bash
kubectl apply -f https://github.com/jetstack/cert-manager/releases/latest/download/cert-manager.yaml
helm repo add capi-operator https://kubernetes-sigs.github.io/cluster-api-operator
helm repo update
helm upgrade --install capi-operator capi-operator/cluster-api-operator --create-namespace -n capi-operator-system --set infrastructure=docker --set addon=fleet

# Apply CAAPF URL patch
kubectl patch addonprovider fleet -n fleet-addon-system --type='merge' -p '{"spec": {"fetchConfig": {"url": "https://github.com/rancher-sandbox/cluster-api-addon-provider-fleet/releases/latest/addon-components.yaml"}}}'
```

## Configuration

By default `CAAPF` expects your cluster to have fleet pre-installed and configured, but it can manage installation via `FleetAddonConfig`:

```yaml
apiversion: addons.cluster.x-k8s.io/v1alpha1
kind: FleetAddonConfig
metadata:
  name: fleet-addon-config
spec:
  config:
    server:
      inferLocal: true # Uses default `kuberenetes` endpoint and secret for APIServerURL configuration
  install:
    version: v0.12.0-alpha.6 # We will install alpha for helmapp support
```

You can also define your `API` server `URL` and cerfificates config map, which has a `ca.crt` key:

```yaml
apiversion: addons.cluster.x-k8s.io/v1alpha1
kind: FleetAddonConfig
metadata:
  name: fleet-addon-config
spec:
  config:
    server:
      apiServerUrl: "https://public-url.io"
      apiServerCaConfigRef:
        apiVersion: v1
        kind: ConfigMap
        name: kube-root-ca.crt
        namespace: default
  install:
    version: v0.12.0-alpha.6
```


## Get in contact

You can get in contact with us via the [#cluster-api](https://rancher-users.slack.com/archives/C060L985ZGC) channel on the [Rancher Users Slack](https://slack.rancher.io/).
