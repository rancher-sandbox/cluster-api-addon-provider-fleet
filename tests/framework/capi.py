"""
Functions related to Custer API
"""

import logging

from pathlib import Path
from typing import Optional

from tests.framework.run import run_cmd


class ManagementCluster:
    """
    Manage a CAPI management cluster
    """

    def __init__(
        self,
        kubeconfig: Optional[Path] = None,
    ):
        if kubeconfig is None:
            raise Exception("Kubeconfig is required to init a management cluster")
        self.kubeconfig = kubeconfig

    def init(
        self,
        infra: list[str] = None,
        boostrap: list[str] = None,
        controlplane: list[str] = None,
    ):
        """
        Initialize a management cluster
        """

        init_cmd = f"clusterctl init --kubeconfig={self.kubeconfig}"

        if infra:
            init_cmd += " -i "
            for i in infra:
                init_cmd += f"{i} "
        if boostrap:
            init_cmd += " -b"
            for b in boostrap:
                init_cmd += f"{b} "
        if controlplane:
            init_cmd += " -c"
            for c in controlplane:
                init_cmd += f"{c} "

        logging.info("Creating capi management cluster")
        run_cmd(init_cmd)
        logging.info("Created capi management cluster")
