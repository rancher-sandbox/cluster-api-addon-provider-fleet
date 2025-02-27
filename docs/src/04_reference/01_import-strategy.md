# Import strategy

CAAPF is following simple import strategy for CAPI clusters.
1. Per each CAPI cluster, there is a Fleet `Cluster` object
2. Per each CAPI Cluster Class there is a Fleet `ClusterGroup` object.
3. There is a default `ClusterGroup` for all `ClusterClasses` in the managmement cluster.
4. There is a default `ClusterGroup` for all CAPI `Clusters` in the management cluster.
5. For each CAPI `Cluster` referencing a `ClusterClass` in a different namespace, a `ClusterGroup` is created in the `Cluster` namespace. This `ClusterGroup` targets all clusters in this namespace, pointing to the same `ClusterClass`.

**By default, `CAAPF` imports all `CAPI` clusters under fleet management. See next section for configuration**

![CAAPF-import-groups excalidraw dark](https://github.com/rancher-sandbox/cluster-api-addon-provider-fleet/assets/32226600/0e0bf58d-7030-491e-976e-8363023f0c88)

## Label synchronization

Fleet mainly relies on `Cluster` labels, `Cluster` names and `ClusterGroups` when performing target matching for the desired application or repo content deployment. For that reason `CAAPF` synchronizes labels from the `CAPI` clusters to the imported `Fleet` Cluster resource.

## Configuration

`FleetAddonConfig` provides several configuration options to define clusters to import.

**Note: Please be aware that chaning selection configuration requires restart of the `CAAPF` instance, as these selection options directly translate into watch configurations for controllers established on the `API` server.**

### Namespace Label Selection

This section defines how to select namespaces based on specific labels. The `namespaceSelector` field ensures that the import strategy applies only to namespaces that have the label `import: "true"`. This is useful for scoping automatic import to specific namespaces rather than applying it cluster-wide.

```yaml
apiVersion: addons.cluster.x-k8s.io/v1alpha1
kind: FleetAddonConfig
metadata:
  name: fleet-addon-config
spec:
  cluster:
    namespaceSelector:
      matchLabels:
        import: "true"
```

### Cluster Label Selection

This section filters clusters based on labels, ensuring that the FleetAddonConfig applies only to clusters with the label `import: "true"`. This allows more granular per-cluster selection across the cluster scope.

```yaml
apiVersion: addons.cluster.x-k8s.io/v1alpha1
kind: FleetAddonConfig
metadata:
  name: fleet-addon-config
spec:
  cluster:
    selector:
      matchLabels:
        import: "true"
```
