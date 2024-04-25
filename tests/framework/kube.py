"""
Kubernetes related helpers
"""

import time

from kubernetes import client
from kubernetes.client.rest import ApiException

from framework.timeout import Timeout


def wait_for_deployment_ready(
    api_client: client.ApiClient, name, namespace, timeout=60
):
    apps_v1 = client.AppsV1Api(api_client=api_client)
    with Timeout(
        seconds=timeout, msg=f"waiting for {name} ({namespace}) deployment to be ready"
    ):
        while True:
            response = None
            try:
                response = apps_v1.read_namespaced_deployment_status(name, namespace)
            except ApiException as e:
                if e.status != 404:
                    raise e

            if response:
                status = response.status

                if status.ready_replicas:
                    if status.ready_replicas > 0:
                        return True
            time.sleep(1)
