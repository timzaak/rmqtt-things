# Developing with AI

Development on this project runs on [web-dev-skills](https://github.com/timzaak/web-dev-skills), a standalone AI-development plugin. It chains "decision → PRD / tech research → design → task → development → acceptance → demo" into a resumable pipeline. The workflow itself is not bundled in this repo — the repo keeps the agent rules (`AGENTS.md`) and the runtime artifacts under `.ai/`.

If you just want to add a feature, follow the steps below. You don't need to understand the full architecture.

## Prerequisites

- An AI coding agent with the web-dev-skills plugin loaded (see below)
- MCP Server `context7` configured (used to query Rust/React library docs)
- Project compiles and runs (see [Quick Start](getting-started-en.md))

## Loading web-dev-skills

web-dev-skills is a generic plugin — the same workflow works in any Rust + React project.

- Claude Code: start the project with `claude --plugin-dir /path/to/skills`
- Other agents (ZCode, Codex, …): place a dispatcher skill under `~/.agents/skills/` that routes commands to the cloned repository — see [Using t-tools in Other AI Coding Tools](https://github.com/timzaak/web-dev-skills/blob/main/human/use-in-other-agents.en.md)

Full installation instructions: [timzaak/web-dev-skills](https://github.com/timzaak/web-dev-skills).

## Shortest Loop: Add a New Feature

Say you want to add a "firmware version management" feature called `firmware-version`.

```shell
# 1. Draft the PRD and user stories
t-prd firmware-version

# 2. Generate the technical design
t-design firmware-version

# 3. Generate tasks and execute the backend phase
t-task firmware-version --phase backend
t-run firmware-version --phase backend

# 4. Repeat for the frontend phase
t-task firmware-version --phase frontend
t-run firmware-version --phase frontend

# 5. Run the E2E demo test and accept it
t-web-demo-run demo/e2e/firmware-version-demo.e2e.ts
t-web-demo-accept <role>

# 6. Publish the PRD after acceptance
t-prd-publish firmware-version
```

Each command reads the output of the previous step: `t-design` reads the PRD from `t-prd`, and `t-run` reads the task plan from `t-task`.

Command invocation differs by agent: Claude Code runs them as slash commands (`/t-prd`); agents without plugin support route them through a dispatcher (`/t-tool t-prd`).

`t-prd-check`, `t-design-check`, and `t-task-check` are optional quality gates — run them by risk when a stage's output looks shaky.

## Command Reference

### Requirements and Design

| Command | Args | Output | What it does |
|---------|------|--------|-------------|
| `t-decision <name>` | Feature name | `.ai/decision/` | Product decision gate; routes to tech research or PRD |
| `t-tech-research <name>` | Topic | `.ai/tech-research/` | Feasibility / dependency / cost research before design |
| `t-prd <name>` | Feature name | `.ai/prd/`, `.ai/user-stories/` | Create or update PRD and user-story drafts |
| `t-prd-check <name>` | Feature name | `.ai/quality/` | Validate PRD completeness and story quality |
| `t-prd-publish <name>` | Feature name | `docs/prd/` | Publish the accepted PRD formally |
| `t-design <name>` | Feature name | `.ai/design/` | Generate technical design (API, DB schema, plan) |
| `t-design-check <name>` | Feature name | `.ai/quality/` | Score design implementability |

### Tasks and Execution

| Command | Args | Output | What it does |
|---------|------|--------|-------------|
| `t-task <name> --phase <backend\|frontend>` | Feature + phase | `.ai/task/<name>/` | Break the design into executable tasks for a phase |
| `t-task-check <name>` | Feature name | `.ai/quality/` | Validate task plan executability |
| `t-run <name> --phase <backend\|frontend>` | Feature + phase | `.ai/task/<name>/` | Drive sub-agents to execute the phase |
| `t-super-run <name> --phase <phase>` | Feature + phase | `.ai/super-run/` | Single-session path for strong models: merges planning and execution |

### Testing and Acceptance

| Command | Args | Output | What it does |
|---------|------|--------|-------------|
| `t-web-demo-run <file>` | Path under `demo/e2e/`, e.g. `demo/e2e/alarm-rules-demo.e2e.ts` | `.ai/quality/` | Run a single Playwright E2E test |
| `t-web-demo-run-all` | Optional `continue` | `.ai/quality/` | Run all E2E tests |
| `t-web-demo-accept <role>` | Role name | `.ai/quality/` | Accept test coverage and runnability |

web-dev-skills also covers stacks this project doesn't use (Figma, miniapp, Flutter) — see the plugin README for the full command list.

## Practical Tips

### Run phases sequentially, not all at once

Backend first, then frontend: the frontend depends on the backend's OpenAPI schema. After backend changes, regenerate it with:

```shell
cd frontend && npm run generate-api
```

### PRDs can be iterated

If you find gaps after running `t-prd`, run it again — it reads the existing PRD and updates incrementally without overwriting your manual edits.

### When demo tests fail

Re-run the failing test with `t-web-demo-run`; it auto-diagnoses and attempts fixes. If that doesn't help, read the diagnosis report under `.ai/quality/` — it tells you whether the failure is frontend, backend, or the test itself.

## File Locations

```
AGENTS.md              Agent rules for this repo
.ai/prd/               PRD drafts (published to docs/prd/ by t-prd-publish)
.ai/user-stories/      User-story drafts
.ai/design/            Technical design docs
.ai/task/              Task plans and execution state
.ai/quality/           Check, demo, and acceptance reports
docs/prd/              Published PRDs
demo/e2e/              Playwright E2E tests
frontend/api.json      Exported OpenAPI schema from the backend
```

## Just Fixing a Bug

For small fixes, skip the pipeline. Describe the change to your AI agent directly — it follows the rules in `AGENTS.md`.
