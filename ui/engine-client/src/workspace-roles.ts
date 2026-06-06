/**
 * Workspace roles API — mirrors `engine/houston-engine-server` orchestration routes.
 */

import type { HoustonClient } from "./client";
import type { SessionStartResponse, WorkspaceRoles } from "./types";

export type {
  DataProvision,
  OrchestrateProcedureRequest,
  Procedure,
  Role,
  WorkspaceRoles,
} from "./types";

export function getWorkspaceRoles(
  client: HoustonClient,
  workspaceId: string,
): Promise<WorkspaceRoles> {
  return client.getWorkspaceRoles(workspaceId);
}

export function putWorkspaceRoles(
  client: HoustonClient,
  workspaceId: string,
  roles: WorkspaceRoles,
): Promise<WorkspaceRoles> {
  return client.putWorkspaceRoles(workspaceId, roles);
}

export function startOrchestratedProcedure(
  client: HoustonClient,
  workspaceId: string,
  agentName: string,
  procedureId: string,
  prompt?: string,
): Promise<SessionStartResponse> {
  return client.startOrchestratedProcedure(
    workspaceId,
    agentName,
    procedureId,
    prompt,
  );
}
