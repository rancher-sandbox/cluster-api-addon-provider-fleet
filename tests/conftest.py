"""
Common setup for the start of the test session
"""

import tempfile
import pytest
import time

from kubernetes import config, dynamic, client
# from kubernetes.client import api_client

from framework.kind import KindCluster
from framework.capi import ManagementCluster
from framework.timeout import Timeout
from framework.kube import wait_for_deployment_ready


def pytest_addoption(parser):
    """
    Add command line options.
    """
    parser.addoption(
        "--mgmt-k8s-version",
        action="store",
        default="v1.27.0",
        help="the k8s version of the management cluster",
    )
    parser.addoption(
        "--fmt-config", action="store", help="the path to the rustfmt config"
    )


@pytest.fixture(scope="session", autouse=True)
def test_run_path():
    """
    Creates a temporary directory for the test session
    """
    test_path = tempfile.mkdtemp(prefix="caapf-")
    yield test_path


@pytest.fixture
def kind_cluster(test_run_path):
    """
    Create a kind cluster for the test
    """
    cluster = KindCluster("mgmt", kubeconfig=f"{test_run_path}/mgmt.kubeconfig")
    cluster.create()
    yield cluster
    cluster.destroy()


@pytest.fixture
def kind_api_client(kind_cluster):
    kube_client = config.new_client_from_config(kind_cluster.kubeconfig_path)
    yield kube_client


@pytest.fixture
def mgmt_cluster(kind_cluster, kind_api_client):
    """
    Create a CAPI management cluster
    """
    mgmt = ManagementCluster(kubeconfig=kind_cluster.kubeconfig_path)
    mgmt.init(infra=["docker"])

    wait_for_deployment_ready(
        kind_api_client, "capd-controller-manager", "capd-system", 60
    )
    wait_for_deployment_ready(
        kind_api_client,
        "capi-kubeadm-bootstrap-controller-manager",
        "capi-kubeadm-bootstrap-system",
        60,
    )
    wait_for_deployment_ready(
        kind_api_client,
        "capi-kubeadm-control-plane-controller-manager",
        "capi-kubeadm-control-plane-system",
        60,
    )


@pytest.fixture
def rustfmt_config(pytestconfig):
    """
    Read the rustfmt config file
    """
    config = pytestconfig.getoption("--fmt-config", None)
    if config is None:
        pytest.exit("Please provide path to the rustfmt confif using --fmt-config")
    # data = open(config, encoding="utf-8").read().replace("\n", ",")
    yield config
