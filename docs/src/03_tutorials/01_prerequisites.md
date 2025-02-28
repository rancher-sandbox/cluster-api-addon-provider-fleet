# Prerequisites

## Requirements

- [helm](https://helm.sh/)
- [CAPI management cluster](https://cluster-api.sigs.k8s.io/).
    - Features `EXP_CLUSTER_RESOURCE_SET` and `CLUSTER_TOPOLOGY` must be enabled.
    - It is recommend to use `KUBE_VERSION` >= 1.26.3.
    - [clusterctl](https://cluster-api.sigs.k8s.io/user/quick-start.html?highlight=clusterctl#install-clusterctl).

## Installation

As this project is still in experimental mode, we recommend you start by installing the provider into a kind development cluster specific for this purpose, so you can interact with it in a safe test environment. **CAUTION: while features of this project are marked as experimental, you could experience unexpected failures**.

### Create your local cluster

> NOTE: if you prefer to opt for a one-command installation, you can refer to the notes on how to use `just` and the project's `justfile` [here](../developers/development.md).

1. Start by adding the helm repositories that are required to proceed with the installation.
```
helm repo add fleet https://rancher.github.io/fleet-helm-charts/
helm repo update
```
2. Navigate to [kind-config.yaml](../../../../testdata/kind-config.yaml) and inspect the kind cluster configuration file, that includes a `LOCAL_IP` environment variable that we'll be setting next, based on your local networking configuration.
```
export LOCAL_IP=$(ip -4 -j route list default | jq -r .[0].prefsrc)
envsubst < testdata/kind-config.yaml > _out/kind-config.yaml
```
3. Create the local cluster. It is recommended to use `KUBE_VERSION>=1.26.3`.
```
kind create cluster --image=kindest/node:v{{KUBE_VERSION}} --config _out/kind-config.yaml
```
4. Install [fleet](https://github.com/rancher/fleet) and specify the `API_SERVER_URL` and CA.
```
# We start by retrieving the CA data from the cluster
kubectl config view -o json --raw | jq -r '.clusters[] | select(.name=="kind-dev").cluster["certificate-authority-data"]' | base64 -d > _out/ca.pem
# Set the API server URL
API_SERVER_URL=`kubectl config view -o json --raw | jq -r '.clusters[] | select(.name=="kind-dev").cluster["server"]'`
# And proceed with the installation via helm
helm -n cattle-fleet-system install --version v0.12.0-alpha.3 --create-namespace --wait fleet-crd fleet/fleet-crd
helm install --create-namespace --version v0.12.0-alpha3 -n cattle-fleet-system --set apiServerURL=$API_SERVER_URL --set-file apiServerCA=_out/ca.pem fleet fleet/fleet --wait
```
5. Install CAPI with the required experimental features enabled and initialized the Docker provider for testing.
```
EXP_CLUSTER_RESOURCE_SET=true CLUSTER_TOPOLOGY=true clusterctl init -i docker
```

Wait for all pods to become ready and your cluster should be ready to use CAAPF!

**Remember that you can follow along with the video demo to install the provider and get started quickly.**

[![asciicast](https://asciinema.org/a/659626.svg)](https://asciinema.org/a/659626)

