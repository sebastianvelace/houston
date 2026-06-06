import { DropdownMenuItem } from "@houston-ai/core";
import type { SidebarItem } from "@houston-ai/layout";
import type { WorkspaceRoles } from "@houston-ai/engine-client";
import type { Agent } from "../../lib/types";
import { agentRoleBadges, roleForAgentName } from "../../lib/workspace-roles";
import { AgentSidebarColorMenu } from "./agent-sidebar-color-menu";
import type { AgentActivitySummary } from "./agent-activity-summary-model";
import type { OrchestrationSetupReason } from "../orchestration/orchestration-setup-hint";
import { AgentRoleBadges } from "./agent-role-badges";
import { AgentSidebarIcon, NeedsYouChip } from "./agent-sidebar-status";

interface BuildAgentSidebarItemsArgs {
  agents: Agent[];
  /** Hidden from "Your agents"; lives in Executive Manager only. */
  executiveAgentName?: string;
  workspaceRoles?: WorkspaceRoles;
  summaries: Record<string, AgentActivitySummary>;
  runningLabel: (count: number) => string;
  needsYouLabel: (count: number) => string;
  onChangeColor: (agentId: string, color: string) => void;
  onShareAgent: (agentId: string) => void;
  shareLabel: string;
}

export function buildAgentSidebarItems({
  agents,
  executiveAgentName,
  workspaceRoles,
  summaries,
  runningLabel,
  needsYouLabel,
  onChangeColor,
  onShareAgent,
  shareLabel,
}: BuildAgentSidebarItemsArgs): SidebarItem[] {
  const visibleAgents = executiveAgentName
    ? agents.filter((agent) => agent.name !== executiveAgentName)
    : agents;

  return visibleAgents.map((agent) => {
    const summary = summaries[agent.id] ?? {
      needsYouCount: 0,
      runningCount: 0,
    };
    const hasRunning = summary.runningCount > 0;
    const role = roleForAgentName(workspaceRoles, agent.name);
    const badges = agentRoleBadges(role);
    let setupHint: OrchestrationSetupReason | null = null;
    if (workspaceRoles && !role) {
      setupHint =
        workspaceRoles.roles.length === 0 ? "no_roles" : "unassigned";
    }

    return {
      id: agent.id,
      name: agent.name,
      icon: (
        <AgentSidebarIcon
          color={agent.color}
          running={hasRunning}
          runningLabel={runningLabel(summary.runningCount)}
        />
      ),
      trailing: (
        <div className="flex items-center gap-1">
          <AgentRoleBadges
            roleName={badges.roleName}
            isProvider={badges.isProvider}
            isOrchestrator={badges.isOrchestrator}
            setupHint={setupHint}
          />
          {summary.needsYouCount > 0 ? (
            <NeedsYouChip
              count={summary.needsYouCount}
              label={needsYouLabel(summary.needsYouCount)}
            />
          ) : null}
        </div>
      ),
      menuContent: (
        <>
          <AgentSidebarColorMenu
            color={agent.color}
            onChange={(color) => onChangeColor(agent.id, color)}
          />
          <DropdownMenuItem onClick={() => onShareAgent(agent.id)}>
            {shareLabel}
          </DropdownMenuItem>
        </>
      ),
    };
  });
}
