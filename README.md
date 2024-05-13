# Cluster API Add-on Provider for Fleet

> NOTE: this is a work in progress. The project is looking for more contributors.

## What is Cluster API Add-on Provider for Fleet (CAAPF)?

Cluster API Add-on Provider for Fleet (CAAPF) is a Cluster API (CAPI) provider that provides integration with [Fleet](https://github.com/rancher/fleet) to enable the easy deployment of applications to a CAPI provisioned cluster.

It provides the following functionality:

- The provider will register a newly provisioned CAPI cluster with Fleet so that applications can be automatically deployed to the created cluster using GitOps.
- The provider will automatically create a [Fleet Cluster Group](https://fleet.rancher.io/cluster-group) for every [CAPI ClusterClass](https://cluster-api.sigs.k8s.io/tasks/experimental-features/cluster-class/). This enables you to deploy the same applications to all clusters created from the same ClusterClass.

## Getting started

Coming soon!

## Get in contact

You can get in contact with us via the [#cluster-api](https://rancher-users.slack.com/archives/C060L985ZGC) channel on the [Rancher Users Slack](https://slack.rancher.io/).
