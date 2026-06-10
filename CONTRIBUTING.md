# Contributing

Use Python 3.11 or newer.

```bash
python3.11 -m venv .venv
. .venv/bin/activate
pip install -e '.[dev]'
ruff check .
pytest
```

Keep provider APIs, credentials, automatic approvals, merge behavior, deployment, and release
decisions out of RepoTask core. Add project-specific behavior through focused stack profiles or
provider/VCS adapters rather than generic assumptions.

