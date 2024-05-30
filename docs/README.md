# Documentation Index

## Quick start
Before starting your journey with CAAPF, you may want to familiarize with CAPI and the concept of providers.
- [Cluster API Quick Start](https://cluster-api.sigs.k8s.io/user/quick-start.html)
Deploy your own CAPI management cluster and install the Add-on Provider for Fleet.
- [Getting started with CAAPF](./book/src/topics/index.md)

## Features
- [Roadmap](./book/src/roadmap.md)

## Development
If you are a developer, the project provides a series of commands that you can use to configure your local environment and operate with the provider.
- [Development Guide](./book/src/developers/development.md)
- [Releasing](./book/src/developers/release.md)

> Remember that this is a work in progress and the project is looking for more contributors.

## What is Cluster API Add-on Provider for Fleet (CAAPF)?

Cluster API Add-on Provider for Fleet (CAAPF) is a Cluster API (CAPI) provider that provides integration with [Fleet](https://github.com/rancher/fleet) to enable the easy deployment of applications to a CAPI provisioned cluster.

It provides the following functionality:

- The provider will register a newly provisioned CAPI cluster with Fleet so that applications can be automatically deployed to the created cluster using GitOps.
- The provider will automatically create a [Fleet Cluster Group](https://fleet.rancher.io/cluster-group) for every [CAPI ClusterClass](https://cluster-api.sigs.k8s.io/tasks/experimental-features/cluster-class/). This enables you to deploy the same applications to all clusters created from the same ClusterClass.

## Demo

[![asciicast](https://asciinema.org/a/659626.svg)](https://asciinema.org/a/659626)

## Getting started

You can refer to the rest of the provider documentation [here](./docs/book/src/.).

## Get in contact

You can get in contact with us via the [#cluster-api](https://rancher-users.slack.com/archives/C060L985ZGC) channel on the [Rancher Users Slack](https://slack.rancher.io/).
