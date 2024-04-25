"""Tests for styling rust"""

from framework import run

def test_rust_style(rustfmt_config):
    """
    Test that rust code passes style checks.
    """

    _, stdout, _ = run.run_cmd(f"cargo fmt --all -- --check --config-path {rustfmt_config}")
    assert "Diff in" not in stdout
