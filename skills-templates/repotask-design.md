---
name: repotask-design
description: Read a Figma design's structure and find which of this project's existing components already implement it, through the repo-task CLI. Use when implementing a screen or component from a design, when handed a Figma link or node id, or when asked whether a design element already exists in code. Triggers include "build this screen", "implement this design", "from Figma", "does this component exist".
---

# Design to code via repo-task

## 1. Read the design structure

```bash
repo-task --json design node <node-id> --depth 5
repo-task --json design file            # the whole file, shallower
repo-task --json design variables       # design tokens: colour, spacing, type
```

The CLI fetches over Figma's API and returns only what matters for building — frames,
components, text, layout, and variables. Transform matrices and vector geometry are
dropped, so you get the structure without the payload.

If the CLI cannot reach Figma it returns a tool call instead; make it through your own
Figma connection, then continue below.

## 2. To actually look at the design

```bash
repo-task --json design image <node-id>
```

Returns rendered URLs. Open them — the CLI cannot see a design, you can.

## 3. Map it onto this project — do this before writing a component

```bash
repo-task --json design map "Button/Primary" "FeedCard" "Checkout/Sheet"
```

Returns, for each design component name: the code component that already implements it,
the files involved, this project's conventions for building screens, and the names with
no implementation yet.

## Rules

- Run `design map` before creating any component. A design system's `Button/Primary` is
  usually already built here under some other name, and a second implementation is worse
  than an imperfect reuse.
- Build only what comes back under `unmatched`. Name it the way its siblings are named.
- Use the token names from `design variables` rather than raw hex and pixel values.
- When the design and the existing component disagree about shape or behaviour, say so.
  Do not silently reshape the component to match a single screen.
- The design is not the source of truth for architecture; the conventions in the returned
  context are. Follow both, and flag the conflict when they pull apart.
