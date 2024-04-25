"""
Check the style of the python code files.
"""
import sys
from subprocess import run

def test_pylint():
    """
    Lint the python code
    """
    cmd = [
        "pylint --jobs=0 --persistent=no --score=no "
        '--output-format=colorized --attr-rgx="[a-z_][a-z0-9_]{1,30}$" '
        '--argument-rgx="[a-z_][a-z0-9_]{1,35}$" '
        '--variable-rgx="[a-z_][a-z0-9_]{1,30}$" --disable='
        "fixme,too-many-instance-attributes,import-error,"
        "too-many-locals,too-many-arguments,consider-using-f-string,"
        "consider-using-with,implicit-str-concat,line-too-long,redefined-outer-name,"
        "broad-exception-raised,duplicate-code tests"
    ]

    run(
        cmd,
        stdout=sys.stdout,
        stderr=sys.stderr,
        shell=True,
        cwd="..",
        check=True
    )
