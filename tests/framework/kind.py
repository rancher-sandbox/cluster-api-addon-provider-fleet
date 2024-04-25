"""
Kind related functions
"""
import logging
import subprocess
import tempfile

from pathlib import Path
from typing import Optional, Union

class KindCluster:
    """
    Represents a KinD cluster.
    """
    def __init__(self,
                 name: str,
                 kubeconfig: Optional[Path] = None,
                 config_file: Optional[Union[str, Path]] = None
                 ):
        self.name = name
        self.kubeconfig_path = kubeconfig or (tempfile.NamedTemporaryFile(prefix='mgmt_', suffix='.kubeconfig'))
        self.config_file = None
        if config_file:
            self.config_file = str(config_file)


    def create(self,):
        """Creates a new kind cluster"""
        create_cmd = [
            "kind",
            "create",
            "cluster",
            f"--name={self.name}",
            f"--kubeconfig={self.kubeconfig_path}",
        ]
        if self.config_file:
            create_cmd += ["--config", self.config_file]

        logging.info('Creating cluster %s', self.name)
        subprocess.run(create_cmd, check=True)
        logging.info('Created cluster %s', self.name)

    def destroy(self):
        """
        Destroys the kind cluster if it exists
        """
        if not self.cluster_exists():
            logging.info("cluster doesn't exist, skipping deletion")
            return

        delete_cmd = [
            "kind",
            "delete",
            "cluster",
            f"--name={self.name}",
        ]
        logging.info('Deleteing cluster %s', self.name)
        subprocess.run(delete_cmd, check=True)
        logging.info('Deleted cluster %s', self.name)

    def cluster_exists(self):
        """
        Checks if the kind cluster exists
        """
        out = subprocess.check_output(
            ["kind", "get", "clusters"], encoding="utf-8"
        )
        for name in out.splitlines():
            if name == self.name:
                return True
        return False
