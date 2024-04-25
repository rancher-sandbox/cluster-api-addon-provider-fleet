#!/bin/sh

just
pip install poetry
poetry config virtualenvs.create false
python -m venv .venv --prompt feria
chown -R vscode:vscode .venv
source .venv/bin/activate
pwd
cd tests; poetry install --no-interaction --no-root --no-cache