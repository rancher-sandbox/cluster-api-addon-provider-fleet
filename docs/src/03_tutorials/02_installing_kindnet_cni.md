# Installing Kindnet CNI using resource Bundle

This section describes steps to install `kindnet` CNI solution on a CAPI cluster using Fleet `Bundle` resource.

## Deploying Kindnet

We will use Fleet [`Bundle` resource][bundle] to deploy Kindnet on the docker cluster.

[bundle]: https://fleet.rancher.io/ref-bundle

```bash
> kubectl get clusters
NAME              CLUSTERCLASS   PHASE         AGE   VERSION
docker-demo       quick-start    Provisioned   35h   v1.29.2
```

First, let's review our targes for the kindnet bundle. They should match labels on the cluster, or the name of the cluster, as in this instance:

```yaml
  targets:
  - clusterName: docker-demo
```

We will apply the resource from the:

```yaml
{{#include ../../../testdata/cni.yaml}}
```

```bash
> kubectl apply -f testdata/cni.yaml
bundle.fleet.cattle.io/kindnet-cni configured
```

After some time we should see the resource in a ready state:

```bash
> kubectl get bundles kindnet-cni
NAME          BUNDLEDEPLOYMENTS-READY   STATUS
kindnet-cni   1/1
```

This should result in a `kindnet` running on the matching cluster:

```bash
> kubectl get pods --context docker-demo -A | grep kindnet
kube-system         kindnet-dqzwh                                         1/1     Running   0          2m11s
kube-system         kindnet-jbkjq                                         1/1     Running   0          2m11s
```

### Demo

<script src="https://asciinema.org/a/6x8WmsCXJQdDswAwfYHQlaVsj.js" id="asciicast-6x8WmsCXJQdDswAwfYHQlaVsj" async="true" data-start-at="327"></script>