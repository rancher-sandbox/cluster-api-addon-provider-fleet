# Import strategy

CAAPF is following simple import strategy for CAPI clusters.
1. Per each CAPI cluster, there is a Fleet `Cluster` object
2. Per each CAPI Cluster Class there is a Fleet `ClusterGroup` object.
3. There is a default `ClusterGroup` for all `ClusterClasses` in the managmement cluster.
4. There is a default `ClusterGroup` for all CAPI `Clusters` in the management cluster.

![CAAPF-import-groups excalidraw dark](https://github.com/rancher-sandbox/cluster-api-addon-provider-fleet/assets/32226600/0e0bf58d-7030-491e-976e-8363023f0c88)
