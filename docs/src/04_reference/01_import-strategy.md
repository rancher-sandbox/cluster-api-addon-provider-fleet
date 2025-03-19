# Import Strategy

CAAPF follows a simple import strategy for CAPI clusters:

1. Each CAPI cluster has a corresponding Fleet `Cluster` object.
2. Each CAPI Cluster Class has a corresponding Fleet `ClusterGroup` object.
3. When a CAPI `Cluster` references a `ClusterClass` in a different namespace, a `ClusterGroup` is created in the `Cluster` namespace. This `ClusterGroup` targets all clusters in this namespace that reference the same `ClusterClass`. See the [configuration](#cluster-clustergroupbundlenamespacemapping-configuration) section for details.
4. If at least one CAPI `Cluster` references a `ClusterClass` in a different namespace, a [`BundleNamespaceMapping`][mapping] is created in the `ClusterClass` namespace. This allows Fleet `Cluster` resources to use application sources such as `Bundles`, `HelmApps`, or `GitRepos` from the `ClusterClass` namespace as if they were deployed in the `Cluster` namespace. See the [configuration](#cluster-clustergroupbundlenamespacemapping-configuration) section for details.

[mapping]: https://fleet.rancher.io/namespaces#cross-namespace-deployments

**By default, `CAAPF` imports all `CAPI` clusters under Fleet management. See the next section for configuration details.**

![CAAPF-import-groups excalidraw dark](https://github.com/rancher-sandbox/cluster-api-addon-provider-fleet/assets/32226600/0e0bf58d-7030-491e-976e-8363023f0c88)

## Label Synchronization

Fleet relies on `Cluster` labels, `Cluster` names, and `ClusterGroups` for target matching when deploying applications or referenced repository content. To ensure consistency, `CAAPF` synchronizes resource labels:

1. From the CAPI `ClusterClass` to the imported Fleet `Cluster` resource.
2. From the CAPI `ClusterClass` to the imported Fleet `ClusterGroup` resource.

When a CAPI `Cluster` references a `ClusterClass`, `CAAPF` applies two specific labels to both the `Cluster` and `ClusterGroup` resources:

- `clusterclass-name.fleet.addons.cluster.x-k8s.io: <class-name>`
- `clusterclass-namespace.fleet.addons.cluster.x-k8s.io: <class-ns>`

## Configuration

`FleetAddonConfig` provides several configuration options to define which clusters to import.

### Cluster `ClusterGroup`/`BundleNamespaceMapping` Configuration

When a CAPI `Cluster` references a `ClusterClass` in a different namespace, a corresponding `ClusterGroup` is created in the **`Cluster`** namespace. This ensures that all clusters within the namespace that share the same `ClusterClass` from another namespace are grouped together.

This `ClusterGroup` inherits `ClusterClass` labels and applies two `CAAPF`-specific labels to uniquely identify the group within the cluster scope:

- `clusterclass-name.fleet.addons.cluster.x-k8s.io: <class-name>`
- `clusterclass-namespace.fleet.addons.cluster.x-k8s.io: <class-ns>`

Additionally, this configuration enables the creation of a `BundleNamespaceMapping`. This mapping selects all available bundles and establishes a link between the namespace of the `Cluster` and the namespace of the referenced `ClusterClass`. This allows the Fleet `Cluster` to be evaluated as a target for application sources such as `Bundles`, `HelmApps`, or `GitRepos` from the **`ClusterClass`** namespace.

When all CAPI `Cluster` resources referencing the same `ClusterClass` are removed, both the `ClusterGroup` and `BundleNamespaceMapping` are cleaned up.

To enable this behavior, configure `FleetAddonConfig` as follows:

```yaml
apiVersion: addons.cluster.x-k8s.io/v1alpha1
kind: FleetAddonConfig
metadata:
  name: fleet-addon-config
spec:
  cluster:
    applyClassGroup: true
```

Setting `applyClassGroup: true` ensures that Fleet automatically creates a `ClusterGroup` object for each `Cluster` resource and applies the necessary `BundleNamespaceMapping` for cross-namespace bundle access.

**Note: If the `cluster` field is not set, this setting is enabled by default.**

### Namespace Label Selection

This configuration defines how to select namespaces based on specific labels. The `namespaceSelector` field ensures that the import strategy applies only to namespaces that have the label `import: "true"`. This is useful for scoping automatic import to specific namespaces rather than applying it cluster-wide.

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

This configuration filters clusters based on labels, ensuring that the `FleetAddonConfig` applies only to clusters with the label `import: "true"`. This allows more granular per-cluster selection across the cluster scope.

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
