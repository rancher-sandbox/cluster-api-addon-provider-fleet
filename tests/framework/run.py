"""Utility functions for running commands"""

import logging
import subprocess

from collections import namedtuple

CommandReturn = namedtuple("CommandReturn", "returncode stdout stderr")

def run_cmd_sync(cmd, ignore_return_code=False, no_shell=False, cwd=None, timeout=None):
    """
    Execute a given command.

    :param cmd: command to execute
    :param ignore_return_code: whether a non-zero return code should be ignored
    :param no_shell: don't run the command in a sub-shell
    :param cwd: sets the current directory before the child is executed
    :return: return code, stdout, stderr
    """
    if isinstance(cmd, list) or no_shell:
        # Create the async process
        proc = subprocess.Popen(
            cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, cwd=cwd
        )
    else:
        proc = subprocess.Popen(
            cmd, shell=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, cwd=cwd
        )

    # Capture stdout/stderr
    stdout, stderr = proc.communicate(timeout=timeout)

    output_message = f"\n[{proc.pid}] Command:\n{cmd}"
    # Append stdout/stderr to the output message
    if stdout != "":
        output_message += f"\n[{proc.pid}] stdout:\n{stdout.decode()}"
    if stderr != "":
        output_message += f"\n[{proc.pid}] stderr:\n{stderr.decode()}"

    # If a non-zero return code was thrown, raise an exception
    if not ignore_return_code and proc.returncode != 0:
        output_message += f"\nReturned error code: {proc.returncode}"

        if stderr != "":
            output_message += f"\nstderr:\n{stderr.decode()}"
        raise ChildProcessError(output_message)

    # Log the message with one call so that multiple statuses
    # don't get mixed up
    logging.debug(output_message)

    return CommandReturn(proc.returncode, stdout.decode(), stderr.decode())


def run_cmd(cmd, **kwargs):
    """
    Run a command using the sync function that logs the output.

    :param cmd: command to run
    :returns: tuple of (return code, stdout, stderr)
    """
    return run_cmd_sync(cmd, **kwargs)
