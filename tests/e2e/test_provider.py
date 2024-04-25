"""
Functional tests for the addon provider
"""

import pytest


@pytest.fixture
def local_addon(mgmt_cluster, kind_api_client):
    """
    Install a local version of the addon provider
    local in this context means helm installing thr chart from source code
    and loading the image into kind
    """
    print("TODO")


def test_happy_path(local_addon, kind_api_client):
    """
    Test the happy path execution
    """
    print("TODO: add the actual test")
    if kind_api_client:
        print("we have the management cluster client")
