# Generic Human-Controlled Workflow

1. Confirm the requirement and acceptance criteria.
2. Create a feature branch from the configured base branch.
3. Investigate the affected code and record assumptions.
4. Implement and verify the change manually.
5. Review the diff and prepare the change request.
6. A human reviews and approves the change request.
7. QA, conflict resolution, deployment, and release decisions remain human-controlled.

RepoTask may prepare prompts and deterministic local artifacts. It must not make approval
decisions, merge changes, resolve conflicts, deploy, or release automatically.

