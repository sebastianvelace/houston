<p align="center">
  <a href="https://gethouston.ai">
    <strong>Houston</strong>
  </a>
</p>

<p align="center">
  <strong>The open source platform for AI-native products.</strong><br>
  One desktop app. Pre-built AI agents that work from day one.<br>
  Real tools. 1000+ integrations. Free forever.
</p>

<p align="center">
  <a href="https://gethouston.ai">gethouston.ai</a> ·
  <a href="https://gethouston.ai/vision/">Vision</a> ·
  <a href="https://gethouston.ai/learn/">Learn</a> ·
  <a href="https://gethouston.ai/startups/">For Startups</a> ·
  <a href="https://forms.gle/ac24qrKSufYvfudt8">Join the waiting list</a>
</p>

<p align="center">
  <a href="https://github.com/gethouston/houston/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-0d0d0d" alt="MIT License"></a>
  <a href="https://github.com/gethouston/houston/stargazers"><img src="https://img.shields.io/github/stars/gethouston/houston?color=0d0d0d" alt="Stars"></a>
</p>

---

## What Houston is

**For everyone** — a free desktop app with AI agents that do real work. Bookkeeping, outreach, research, scheduling. Install agents from the store and start working. No terminal. No prompt engineering.

**For founders** — the platform where you build AI-native products for your customers. Define your agents, Houston handles the workspace, the chat, the board, the integrations. You bring the domain expertise. [Read more](https://gethouston.ai/startups/).

> **Read the vision:** [Ship the impossible](https://gethouston.ai/vision/)

---

## Quick start

### Run the Houston app

```bash
git clone https://github.com/gethouston/houston.git
cd houston
pnpm install
cd app && pnpm tauri dev
```

### Build your first agent

Create two files:

**houston.json**
```json
{
  "id": "bookkeeper",
  "name": "Bookkeeper",
  "description": "Categorize expenses and reconcile accounts.",
  "icon": "Calculator",
  "category": "business"
}
```

**CLAUDE.md**
```markdown
# Bookkeeper

You categorize transactions, reconcile accounts, and flag anomalies.
Ask which period the user wants before starting.
```

Push to GitHub. In Houston, click **New Agent > GitHub**, paste your repo URL. Done.

The [Learn guide](https://gethouston.ai/learn/) covers the full details in five short chapters.

### Share a workspace template

Bundle multiple agents into one repo:

```
my-workspace/
├── workspace.json
└── agents/
    ├── bookkeeper/
    │   ├── houston.json
    │   └── CLAUDE.md
    └── tax-reviewer/
        ├── houston.json
        └── CLAUDE.md
```

**workspace.json**
```json
{
  "name": "Tax Practice",
  "description": "A complete workspace for tax professionals.",
  "agents": ["bookkeeper", "tax-reviewer"]
}
```

In Houston, click **New Workspace > Import from GitHub**, paste the repo URL. Houston creates the workspace with all agents ready to use.

---

## How the app works

Houston organizes work into **Workspaces** and **Agents**:

- **Workspace** — a group of agents (like a team or project).
- **Agent** — an AI agent instance. Chat, kanban board, skills, files, integrations.
- **Agent Definition** — a `houston.json` that defines what an agent looks like and does.

```
Workspace ("Tax Practice")
  ├── Agent ("Bookkeeper")         ← board, files, instructions
  ├── Agent ("Document Reviewer")  ← board, files, integrations
  └── Agent ("Client Comms")       ← board, files, integrations
```

Each kanban card is a Claude conversation. Click a card to see the full chat. Connect Slack and the same conversation becomes a thread.

---

## Agent definitions

Two tiers:

| Tier | What you write | What you get |
|------|---------------|-------------|
| **JSON-only** | `houston.json` + `CLAUDE.md` | A new agent. Renders the standard shell (Activity, Routines, Files, Job Description, Integrations). |
| **Workspace template** | `workspace.json` + agents folder | Multiple agents, one import. |

Every agent shows the same five tabs. The list lives in `app/src/agents/standard-tabs.ts` if you want to read it in code.

---

## Monorepo layout

Organized as **6 end-user products + 3 code libraries**.

```
houston/
├── app/                     Houston App — desktop (Tauri 2)
│   ├── src/                 React frontend
│   ├── src-tauri/           Tauri binary
│   └── houston-tauri/       Tauri adapter (applies Engine to desktop)
├── mobile/                  Houston Mobile companion
├── desktop-mobile-bridge/   Cloudflare Worker — pairs Desktop ↔ Mobile
├── store/                   Houston Store — agent registry
├── website/                 Houston Website — gethouston.ai
├── always-on/               Houston Always On — VPS deploy (Dockerfile + compose + systemd)
├── teams/                   Houston Teams (TBD — hosted multi-tenant)
│
├── ui/                      Houston UI — @houston-ai/* React packages
├── engine/                  Houston Engine — Rust crates (frontend-agnostic)
├── cloud/                   Houston Cloud (TBD — managed Engine hosting)
│
└── examples/                Reference consumers of houston-engine
    └── smartbooks/            Bookkeeping app built on a custom React frontend
```

See `knowledge-base/architecture.md` for crate-level detail + current gaps.

---

## Build on Houston Engine (custom frontends)

The engine is frontend-agnostic. You don't have to ship inside the
Houston App — any web or native runtime can drive it over HTTP +
WebSocket using [`@houston-ai/engine-client`](ui/engine-client/).

**Working example: [SmartBooks](examples/smartbooks/)** — a
bookkeeping product with its own brand, its own UX, and zero
`@houston-ai/*` UI deps. ~400 lines of TSX, one npm package, renders
a live transactions table + a multi-sheet Excel workpaper. Soft
workflow: the user asks for a new column, Claude edits the Python
script, every future upload picks up the change. Clone it, rename
things, ship your own AI-native product.

```bash
cd examples/smartbooks
pnpm install
pnpm dev
```

Full walkthrough + architecture diagram + custom-frontend gotchas in
[examples/smartbooks/README.md](examples/smartbooks/README.md).

---

## Resources

- **[gethouston.ai](https://gethouston.ai)** — landing page
- **[For Startups](https://gethouston.ai/startups/)** — build AI-native products on Houston
- **[Vision essay](https://gethouston.ai/vision/)** — Ship the impossible
- **[Learn guide](https://gethouston.ai/learn/)** — five chapters on building agents
- **[Join the waiting list](https://forms.gle/ac24qrKSufYvfudt8)** — get notified when the app ships

---

## Contributing

Houston is open source under MIT. Issues and PRs welcome.

---

## Executive Manager & Sandbox (2026)

Hackathon work shipping **agent isolation** (Persona A/B) and **multi-agent orchestration**
with a demo-ready **Executive Manager** UI. Full design notes live in
[docs/hackathon-agent-orchestration.md](docs/hackathon-agent-orchestration.md).

> **Provider runtime:** Claude sandbox integration is under investigation. Rebuild the
> engine after pulling engine changes; if sessions fail, try
> `HOUSTON_SANDBOX_BACKEND=landlock` or `HOUSTON_SANDBOX=off` while debugging.

### Agent sandbox

Each agent CLI subprocess runs inside an OS-level sandbox enforced by two new engine crates:

- **`houston-policy`** — builds a `SessionPolicy` per session: working directory, extra
  read/write paths, and a **cross-agent denylist** (sibling agents, credentials, workspace
  roots under `~/.houston/`).
- **`houston-sandbox`** — applies the policy on Linux (bubblewrap or Landlock),
  macOS (`sandbox-exec`), and Windows.

**Environment variables**

| Variable | Values | Effect |
|----------|--------|--------|
| `HOUSTON_SANDBOX` | `off` | Disable sandbox entirely |
| | `strict` / `permissive` | Fail closed vs allow missing backend features |
| | unset | Strict in release builds, permissive in debug |
| `HOUSTON_SANDBOX_BACKEND` | `bwrap`, `landlock`, `auto` (Linux) | Pick isolation backend |

**Credential staging** — Claude and Codex CLIs spawn with a per-session staged `HOME`
that symlinks only the minimum auth files (`.credentials.json`, etc.), keeping real
credential trees out of the subprocess filesystem.

**`/v1/shell`** — shell commands run inside the same sandbox when `agentPath` is set
(agent root as cwd + policy).

**`GET /v1/isolation/capabilities`** — reports what the host supports (backend id,
filesystem/network isolation, credential isolation).

**Dev caveat:** the sandbox binds bundled CLI install paths. After engine changes, run
`cargo build -p houston-engine-server` before `pnpm tauri dev` so the sidecar matches
source. Claude sessions may need a fresh engine binary (see provider note above).

### Agent roles orchestration (engine only)

Workspace-level **`roles.json`** (`~/.houston/workspaces/{Workspace}/roles.json`) declares
roles, what data each role **provides**, and **procedures** an orchestrator can run
(with `requires` pointing at other roles' data).

The engine exposes a full orchestrator API: resolve roles, spawn **sync sub-sessions**
against provider agents, then stream the main orchestrator session. The orchestrator
never reads other agents' files directly — context arrives as LLM text only.

**Demo scope:** the Settings roles editor and sidebar role badges were removed for the
hackathon demo. Configure roles via `roles.json` or the REST API; the shipped UI focuses
on Executive Manager instead.

### Executive Manager (Gerente ejecutivo)

Top-level sidebar nav with a **Crown** icon, between **Integraciones** and
**Configuración**:

```
Centro de misiones
Integraciones
Gerente ejecutivo    ← Executive Manager
Configuración
─────────────────
Tus agentes          ← Director hidden here
```

**What it does**

- Auto-creates a **Director** agent per workspace on first open (seeded `CLAUDE.md`,
  standard agent shell).
- Persists **`executive-config.json`** at workspace level: owner picks which agents the
  Director may consult (e.g. Financiero carros, Financiero motos).
- **Chat with Director** — first message in a new session triggers
  `POST /v1/workspaces/{ws}/executive/briefing`.
- **Director is hidden** from the **Tus agentes** list; manage it only from Executive Manager.
- **Parallel sub-sessions** query each connected agent at once; Director synthesizes one
  executive briefing from their responses.
- **Security:** same model as role orchestration — Director's sandbox stays on its own
  `agent_dir`; connected agents answer via isolated sync sessions; no cross-agent file reads.

**Executive Manager layout**

| Left | Right |
|------|-------|
| Checklist of workspace agents — toggle who Director consults | Director chat panel with orchestration progress on the first briefing |

### Demo quick start

```bash
cargo build -p houston-engine-server
cd app && pnpm tauri dev
```

1. Create two agents in a workspace, e.g. **Financiero carros** and **Financiero motos**.
2. Open **Gerente ejecutivo** (Executive Manager) in the sidebar.
3. Check both agents under connected agents.
4. Ask Director for a company summary — e.g. *"¿Cómo va el negocio este mes?"*

Director queries both agents in parallel, then streams a synthesized answer.

### API endpoints (hackathon)

All routes are on the engine under `/v1`.

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/isolation/capabilities` | Host sandbox capabilities |
| POST | `/shell` | Sandboxed shell (`agentPath` + `path` + `command`) |
| GET | `/workspaces/{ws}/roles` | Read `roles.json` |
| PUT | `/workspaces/{ws}/roles` | Write validated `roles.json` |
| POST | `/workspaces/{ws}/agents/{agent}/orchestrate` | Run a procedure (body: `procedure_id`, optional `prompt`) |
| GET | `/workspaces/{ws}/executive-config` | Read executive config; ensures Director exists |
| PUT | `/workspaces/{ws}/executive-config` | Set connected agents |
| POST | `/workspaces/{ws}/executive/briefing` | Parallel sub-sessions + Director synthesis (body: `prompt`, `sessionKey`) |

---

## License

MIT
