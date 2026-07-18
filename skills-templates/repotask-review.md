---
name: repotask-review
description: Review a diff against this project's own architecture conventions and indexed facts using the repo-task CLI. Use when asked to review changes, check a PR or MR, or verify that work follows project standards before it is proposed. Triggers include "review this", "check my changes", "is this ready to merge", "does this follow our conventions".
---

# Review against project conventions via repo-task

Generic review advice is cheap. Review this diff against what *this* project has
decided, which is what the knowledge base holds.

## Pull the applicable rules

```bash
repo-task --json brief "review changes" --changed
```

`--changed` ranks the knowledge by the files actually touched, so conventions tied to
those paths (schemas, migrations, public APIs) surface first.

## Check the change against the index

```bash
repo-task --json symbol <name>     # does this already exist elsewhere?
repo-task --json fact <family>     # does the new code match the established pattern?
```

Use this to catch duplication and drift: a second implementation of something the
index already contains, or a new file that ignores the shape its siblings share.

## Report

Group findings by severity and cite the convention `id` behind each one:

- **Must fix** — breaks a documented convention, or is a correctness or security defect
- **Should fix** — inconsistent with established project patterns
- **Consider** — improvement that is genuinely optional

## Rules

- Cite the convention id for every "must fix". If you cannot cite one, it is an
  opinion — label it as such rather than dressing it up as a project rule.
- Do not rewrite the implementation. Show the smallest snippet that explains a finding.
- Say plainly when the diff looks correct. Manufacturing findings to seem thorough
  wastes the author's time.
- Do not claim tests, QA, or CI passed unless you were shown that evidence.
