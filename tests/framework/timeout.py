"""
Timeout related stuff
"""

import signal


class Timeout:
    """
    Timeout a section of code

    with Timeout(30):
        time.sleep(35)
    """

    def __init__(self, seconds, msg="Timed out"):
        self.seconds = seconds
        self.msg = msg

    def handle_timeout(self, signum, frame):
        raise TimeoutError(self.msg)

    def __enter__(self):
        signal.signal(signal.SIGALRM, self.handle_timeout)
        signal.alarm(self.seconds)

    def __exit__(self, _type, _value, _traceback):
        signal.alarm(0)
