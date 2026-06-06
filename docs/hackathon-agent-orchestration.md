# Agent Roles & Orchestration

Sistema de roles y orquestación multi-agente para Houston. Define qué información puede
proveer cada agente, qué procedimientos puede ejecutar, y cómo un agente orquestador
recopila contexto de múltiples agentes antes de ejecutar un procedimiento.

---

## Concepto central

```
roles.json (workspace level)
       │
       ▼
  [Orchestrator C]
   ├── sync session → [Agent A]  ──► "dame financial_summary"  ──► respuesta
   ├── sync session → [Agent B]  ──► "dame campaign_performance" ──► respuesta
   └── procedure session ──► ejecuta con contexto A+B combinado
```

Cada agente tiene un **rol** asignado. El rol declara:
- Qué información puede **proveer** a un orquestador
- Qué **procedimientos** puede ejecutar

Un agente orquestador no accede a los archivos de otros agentes directamente.
Corre sesiones síncronas contra ellos para obtener la información requerida,
luego ejecuta el procedimiento principal con el contexto combinado.

---

## `roles.json`

### Ubicación

```
~/.houston/workspaces/{Workspace}/roles.json
```

Nivel workspace porque los roles definen relaciones entre agentes de un mismo
workspace. Un workspace sin `roles.json` tiene roles implícitos vacíos (todos
los agentes son autónomos, sin orquestación).

### Schema

```jsonc
{
  "version": 1,
  "roles": [
    {
      "id": "finance",
      "name": "Finanzas",
      "agents": ["Contabilidad", "Tesoreria"],
      "provides": [
        {
          "id": "financial_summary",
          "description": "Resumen financiero del período actual: ingresos, egresos, saldo"
        },
        {
          "id": "budget_status",
          "description": "Estado del presupuesto por área, porcentaje ejecutado"
        },
        {
          "id": "pending_invoices",
          "description": "Facturas pendientes de pago con montos y fechas de vencimiento"
        }
      ],
      "procedures": [
        {
          "id": "reconcile_accounts",
          "description": "Reconcilia cuentas del período actual",
          "requires": []
        },
        {
          "id": "generate_financial_report",
          "description": "Genera reporte financiero consolidado",
          "requires": []
        }
      ]
    },
    {
      "id": "marketing",
      "name": "Marketing",
      "agents": ["Marketing", "Campanas"],
      "provides": [
        {
          "id": "campaign_performance",
          "description": "Performance de campañas activas: reach, conversión, gasto"
        },
        {
          "id": "audience_segments",
          "description": "Segmentos de audiencia activos y sus características"
        }
      ],
      "procedures": [
        {
          "id": "plan_campaign",
          "description": "Planifica nueva campaña dados presupuesto y objetivos",
          "requires": ["finance.budget_status"]
        }
      ]
    },
    {
      "id": "orchestrator",
      "name": "Director",
      "agents": ["Director", "CEO-Asistente"],
      "provides": [],
      "procedures": [
        {
          "id": "monthly_executive_report",
          "description": "Reporte ejecutivo mensual con datos financieros y de marketing",
          "requires": [
            "finance.financial_summary",
            "marketing.campaign_performance"
          ]
        },
        {
          "id": "budget_campaign_alignment",
          "description": "Verifica alineación entre presupuesto disponible y campañas planificadas",
          "requires": [
            "finance.budget_status",
            "finance.pending_invoices",
            "marketing.campaign_performance"
          ]
        }
      ]
    }
  ]
}
```

### Campos

| Campo | Tipo | Descripción |
|-------|------|-------------|
| `version` | `number` | Schema version. Hoy: `1`. |
| `roles[].id` | `string` | Identificador único del rol en el workspace |
| `roles[].name` | `string` | Nombre legible |
| `roles[].agents` | `string[]` | Nombres de agentes con este rol (deben existir en el workspace) |
| `roles[].provides[].id` | `string` | Identificador del dato que este rol puede proveer |
| `roles[].provides[].description` | `string` | Descripción del dato — se inyecta como instrucción a la sesión proveedora |
| `roles[].procedures[].id` | `string` | Identificador del procedimiento |
| `roles[].procedures[].description` | `string` | Descripción del procedimiento — se inyecta al agente ejecutor |
| `roles[].procedures[].requires` | `string[]` | Lista de `"{role_id}.{provides_id}"` que el orquestador debe recopilar antes de ejecutar |

---

## Flujo de orquestación

Cuando el engine recibe `POST /v1/workspaces/{ws}/agents/{agent}/orchestrate` con `procedure_id`:

```
1. Leer roles.json del workspace
2. Verificar que el agente tiene un rol con el procedure_id solicitado
3. Resolver requires[] → lista de (role_id, provides_id) necesarios
4. Para cada provides requerido:
   a. Resolver qué agentes tienen ese rol
   b. Elegir el primer agente disponible (no en sesión activa)
   c. Construir prompt de consulta usando provides[].description
   d. Spawn sesión síncrona contra ese agente
   e. Esperar respuesta final (no streaming al cliente)
   f. Guardar respuesta como contexto
5. Construir prompt enriquecido:
   - procedures[].description como objetivo
   - Contexto de cada sub-sesión etiquetado por provides_id
6. Spawn sesión principal contra el agente orquestador
7. Streamear resultado al cliente normalmente
```

### Ejemplo concreto

Procedimiento `monthly_executive_report` en agente `Director`:

```
requires: [finance.financial_summary, marketing.campaign_performance]

→ Sub-sesión 1: Contabilidad
  prompt: "Proporciona el financial_summary: Resumen financiero del período
           actual: ingresos, egresos, saldo. Responde solo con los datos,
           sin contexto adicional."
  → esperar respuesta
  → guardar como finance.financial_summary

→ Sub-sesión 2: Marketing
  prompt: "Proporciona el campaign_performance: Performance de campañas
           activas: reach, conversión, gasto. Responde solo con los datos."
  → esperar respuesta
  → guardar como marketing.campaign_performance

→ Sesión principal: Director
  prompt: "[Contexto finance.financial_summary]
           {respuesta de Contabilidad}

           [Contexto marketing.campaign_performance]
           {respuesta de Marketing}

           Procedimiento: monthly_executive_report — Reporte ejecutivo mensual
           con datos financieros y de marketing.
           Ejecuta el procedimiento con el contexto anterior."
  → streamear al cliente
```

---

## Plan de implementación

### Engine — Rust

#### E1. Schema types (`houston-engine-protocol`)

Nuevos tipos en `engine/houston-engine-protocol/src/`:

```rust
// roles.rs
pub struct WorkspaceRoles {
    pub version: u32,
    pub roles: Vec<Role>,
}

pub struct Role {
    pub id: String,
    pub name: String,
    pub agents: Vec<String>,
    pub provides: Vec<DataProvision>,
    pub procedures: Vec<Procedure>,
}

pub struct DataProvision {
    pub id: String,
    pub description: String,
}

pub struct Procedure {
    pub id: String,
    pub description: String,
    pub requires: Vec<String>,  // "role_id.provides_id"
}
```

Derivar `Serialize`, `Deserialize`, `JsonSchema`. Añadir a `PROTOCOL_VERSION`.

#### E2. File I/O (`houston-agent-files`)

Nuevo módulo `workspace_roles.rs` en `houston-agent-files`:

```rust
// lee/escribe ~/{workspace}/roles.json
pub fn read_workspace_roles(workspace_path: &Path) -> Result<WorkspaceRoles>
pub fn write_workspace_roles(workspace_path: &Path, roles: &WorkspaceRoles) -> Result<()>
```

Usar `write_file_atomic` existente. Validar con JSON Schema embebido.
Migración: si no existe `roles.json`, retornar `WorkspaceRoles::default()` (vacío).

#### E3. Role resolver (`houston-engine-core`)

Nuevo módulo `engine/houston-engine-core/src/roles/`:

```rust
// resolver.rs
pub struct RoleResolver {
    workspace_path: PathBuf,
    roles: WorkspaceRoles,
}

impl RoleResolver {
    pub fn load(workspace_path: &Path) -> Result<Self>
    pub fn role_for_agent(&self, agent_name: &str) -> Option<&Role>
    pub fn agents_with_role(&self, role_id: &str) -> Vec<&str>
    pub fn resolve_procedure(&self, agent_name: &str, procedure_id: &str)
        -> Result<ResolvedProcedure>
}

pub struct ResolvedProcedure {
    pub procedure: Procedure,
    // cada requires[] expandido a (role_id, provides, agente_a_consultar)
    pub data_requests: Vec<DataRequest>,
}

pub struct DataRequest {
    pub role_id: String,
    pub provides: DataProvision,
    pub target_agent: String,     // agente elegido para satisfacer el request
    pub target_agent_dir: PathBuf,
}
```

#### E4. Sync session runner (`houston-engine-core`)

Nuevo módulo `engine/houston-engine-core/src/roles/sync_session.rs`:

```rust
/// Corre una sesión contra un agente y espera la respuesta final (no streaming).
/// Timeout configurable; default 120s.
/// Retorna el texto de la última respuesta del LLM.
pub async fn run_sync_session(
    engine_state: &EngineState,
    workspace: &str,
    agent_name: &str,
    prompt: &str,
    timeout: Duration,
) -> Result<String, SyncSessionError>
```

Internamente usa el mismo `session_runner::spawn_and_monitor` existente pero:
- Crea un `mpsc` channel interno (no expuesto al WS del cliente)
- Acumula `SessionUpdate::Feed` hasta recibir `SessionUpdate::Status(Done | Error)`
- Extrae el texto del último `FeedItem::AssistantMessage`
- Cancela con timeout via `tokio::time::timeout`

Emite `HoustonEvent::OrchestrationSubSessionStarted / Completed` para que el
frontend pueda mostrar progreso.

#### E5. Orchestration coordinator (`houston-engine-core`)

Nuevo módulo `engine/houston-engine-core/src/roles/orchestrator.rs`:

```rust
pub async fn run_orchestrated_procedure(
    engine_state: &EngineState,
    workspace: &str,
    agent_name: &str,
    procedure_id: &str,
    user_prompt: Option<&str>,   // contexto adicional del usuario si lo hay
    client_tx: EventSink,        // para streamear la sesión principal
) -> Result<(), OrchestrationError>
```

Implementa el flujo descrito arriba:
1. `RoleResolver::resolve_procedure`
2. Para cada `DataRequest`: `run_sync_session` (secuencial por defecto, paralelo si los agentes son distintos)
3. Construir prompt enriquecido
4. Lanzar sesión principal con `session_runner::spawn_and_monitor` contra el agente orquestador

#### E6. REST endpoint (`houston-engine-server`)

Nuevo route module `engine/houston-engine-server/src/routes/orchestration.rs`:

```
POST /v1/workspaces/{ws}/agents/{agent}/orchestrate
Body: { "procedure_id": "monthly_executive_report", "prompt": "opcional" }
Response: WS session stream (igual que /v1/sessions/start)

GET  /v1/workspaces/{ws}/roles
Response: WorkspaceRoles

PUT  /v1/workspaces/{ws}/roles
Body: WorkspaceRoles
Response: WorkspaceRoles (validado)
```

Registrar en el router de `houston-engine-server`.

#### E7. Security: Landlock para sub-sesiones

Las sub-sesiones síncronas (data providers) usan el `agent_dir` del agente proveedor.
El `SessionPolicy` ya soporta esto desde A1 — no se necesita cambio de schema.

Lo que **sí** cambia: el agente orquestador en la sesión principal **no recibe**
Landlock expandido para leer dirs de otros agentes. La información de los sub-agentes
llega como texto en el prompt, nunca como acceso directo a archivos. El orquestador
queda sandboxed a su propio `agent_dir` como cualquier otro agente.

---

### Frontend — TypeScript/React

#### F1. Tipos TS (`ui/engine-client`)

Espejo de los tipos Rust en `ui/engine-client/src/types.ts`:

```ts
export interface WorkspaceRoles {
  version: number;
  roles: Role[];
}

export interface Role {
  id: string;
  name: string;
  agents: string[];
  provides: DataProvision[];
  procedures: Procedure[];
}

export interface DataProvision {
  id: string;
  description: string;
}

export interface Procedure {
  id: string;
  description: string;
  requires: string[];  // "role_id.provides_id"
}
```

#### F2. Engine client methods (`ui/engine-client`)

```ts
// workspace-roles.ts
export const getWorkspaceRoles = (ws: string): Promise<WorkspaceRoles>
export const putWorkspaceRoles = (ws: string, roles: WorkspaceRoles): Promise<WorkspaceRoles>
export const startOrchestratedProcedure = (
  ws: string, agent: string, procedureId: string, prompt?: string
): Promise<SessionStream>
```

#### F3. WS events

Nuevos eventos en `HoustonEvent`:

```ts
{ type: "OrchestrationSubSessionStarted",  agent: string, provides_id: string }
{ type: "OrchestrationSubSessionCompleted", agent: string, provides_id: string }
{ type: "OrchestrationProcedureStarted",   agent: string, procedure_id: string }
```

Añadir a `use-agent-invalidation.ts` para invalidar queries de sesiones activas.

#### F4. Roles editor (`app/`)

Pantalla de configuración de roles del workspace:
- Lista de roles con sus agentes asignados
- Editor para agregar/editar/eliminar roles
- Asignar agentes a roles (multi-select de agentes del workspace)
- Editor de `provides[]` y `procedures[]` con campo `requires`
- Guardar vía `PUT /v1/workspaces/{ws}/roles`

#### F5. Procedure trigger

En el panel del agente orquestador (agente con `procedures` no vacíos):
- Sección "Procedimientos disponibles" mostrando cada `procedure.id` + descripción
- Botón "Ejecutar" → `POST /v1/.../orchestrate`
- Progress indicator durante sub-sesiones síncronas (usa eventos WS F3)
- La sesión principal streamea en el chat normal

#### F6. Role badge en sidebar

Agentes con rol asignado muestran un badge con el nombre del rol.
Agentes con `provides` no vacíos muestran indicador "proveedor".
Agentes con `procedures` con `requires` muestran indicador "orquestador".

---

## Modelo de seguridad

```
Agente A (rol: finance)          Agente B (rol: marketing)
  Landlock: solo agent_root A      Landlock: solo agent_root B
  ↕ sesión síncrona               ↕ sesión síncroca
       └──────── Agente C (rol: orchestrator) ───────┘
                  Landlock: solo agent_root C
                  Recibe información de A y B SOLO como texto en prompt.
                  NUNCA acceso directo a archivos de A o B.
```

- Las sub-sesiones de A y B corren con su propio `agent_dir` sandboxed
- El contexto fluye como texto plano (respuesta LLM), no como file access
- C no tiene Landlock expandido por ser orquestador
- `roles.json` es leído por el engine (proceso servidor), nunca expuesto como
  archivo al CLI subprocess

---

## Migración de datos

`roles.json` es un archivo nuevo — no hay migración destructiva.
Si no existe: `WorkspaceRoles::default()` (roles vacíos, workspace funciona igual que hoy).
Añadir `migrate_workspace_data` en `houston-agent-files` que crea el archivo vacío
si no existe durante la migración del workspace, para que el filesystem siempre sea
consistente después de actualizar la app.

---

## Archivos a crear/modificar

### Crear
| Archivo | Descripción |
|---------|-------------|
| `engine/houston-engine-protocol/src/roles.rs` | Tipos `WorkspaceRoles`, `Role`, `DataProvision`, `Procedure` |
| `engine/houston-agent-files/src/workspace_roles.rs` | `read_workspace_roles` / `write_workspace_roles` |
| `engine/houston-engine-core/src/roles/mod.rs` | Módulo raíz |
| `engine/houston-engine-core/src/roles/resolver.rs` | `RoleResolver`, `ResolvedProcedure`, `DataRequest` |
| `engine/houston-engine-core/src/roles/sync_session.rs` | `run_sync_session` |
| `engine/houston-engine-core/src/roles/orchestrator.rs` | `run_orchestrated_procedure` |
| `engine/houston-engine-server/src/routes/orchestration.rs` | REST handlers |
| `ui/engine-client/src/workspace-roles.ts` | Engine client methods |
| `app/src/components/workspace/roles-editor.tsx` | UI editor de roles |
| `app/src/locales/en/roles.json` | i18n strings |
| `app/src/locales/es/roles.json` | i18n strings |
| `app/src/locales/pt/roles.json` | i18n strings |

### Modificar
| Archivo | Cambio |
|---------|--------|
| `engine/houston-engine-protocol/src/lib.rs` | Exportar `roles` module |
| `engine/houston-agent-files/src/lib.rs` | Exportar `workspace_roles` module |
| `engine/houston-engine-core/src/lib.rs` | Exportar `roles` module |
| `engine/houston-engine-server/src/routes/mod.rs` | Registrar route `orchestration` |
| `engine/houston-engine-server/src/main.rs` | Añadir rutas de orquestación al router |
| `ui/engine-client/src/index.ts` | Exportar nuevas funciones |
| `ui/engine-client/src/types.ts` | Añadir tipos TS |
| `app/src/hooks/use-agent-invalidation.ts` | Manejar eventos WS de orquestación |
| `app/src/components/agents/agent-panel.tsx` | Sección procedimientos disponibles |
| `app/src/types/react-i18next.d.ts` | Augmentación para namespace `roles` |

---

## Tests requeridos

### Rust
- `roles/resolver.rs`: resolve procedure con requires, agente no encontrado, rol no encontrado, requires mal formado
- `workspace_roles.rs`: round-trip serialización, archivo inexistente retorna default, versión desconocida retorna error
- `roles/orchestrator.rs`: construcción de prompt enriquecido dado contexto mock de sub-sesiones
- `roles/sync_session.rs`: timeout, error de sub-sesión propagado correctamente

### TypeScript
- `workspace-roles.ts`: tipos correctos en request/response
- Roles editor: render con roles vacíos, añadir rol, asignar agente

---

## Orden de implementación recomendado

```
E1 (tipos) → E2 (file I/O) → E3 (resolver) → E4 (sync session) →
E5 (orchestrator) → E6 (REST) → E7 (security check) →
F1+F2 (tipos TS + client) → F3 (WS events) → F4 (roles editor) →
F5 (procedure trigger) → F6 (role badge)
```

E1–E3 son bloqueantes para todo lo demás.
E4 y E5 pueden hacerse en paralelo con F1+F2.
F4–F6 dependen de F1–F3.