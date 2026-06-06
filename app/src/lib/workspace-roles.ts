import type { Procedure, Role, WorkspaceRoles } from "@houston-ai/engine-client";

export const EMPTY_WORKSPACE_ROLES: WorkspaceRoles = {
  version: 1,
  roles: [],
};

export function roleForAgentName(
  roles: WorkspaceRoles | undefined,
  agentName: string,
): Role | null {
  if (!roles) return null;
  return roles.roles.find((role) => role.agents.includes(agentName)) ?? null;
}

export function agentRoleBadges(role: Role | null): {
  roleName: string | null;
  isProvider: boolean;
  isOrchestrator: boolean;
} {
  if (!role) {
    return { roleName: null, isProvider: false, isOrchestrator: false };
  }
  const isOrchestrator = role.procedures.some((procedure) => procedure.requires.length > 0);
  return {
    roleName: role.name,
    isProvider: role.provides.length > 0,
    isOrchestrator,
  };
}

export function proceduresForAgent(
  roles: WorkspaceRoles | undefined,
  agentName: string,
): Procedure[] {
  return roleForAgentName(roles, agentName)?.procedures ?? [];
}

export function allProvidesRefs(roles: WorkspaceRoles): string[] {
  const refs: string[] = [];
  for (const role of roles.roles) {
    for (const provision of role.provides) {
      refs.push(`${role.id}.${provision.id}`);
    }
  }
  return refs;
}

export function newRoleId(existing: Role[]): string {
  let index = existing.length + 1;
  let candidate = `role-${index}`;
  const used = new Set(existing.map((role) => role.id));
  while (used.has(candidate)) {
    index += 1;
    candidate = `role-${index}`;
  }
  return candidate;
}
