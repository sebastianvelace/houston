import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus } from "lucide-react";
import {
  Button,
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyTitle,
  Spinner,
} from "@houston-ai/core";
import type { Role, WorkspaceRoles } from "@houston-ai/engine-client";
import { useAgentStore } from "../../stores/agents";
import { useWorkspaceStore } from "../../stores/workspaces";
import { useUIStore } from "../../stores/ui";
import {
  useSaveWorkspaceRoles,
  useWorkspaceRoles,
} from "../../hooks/queries/use-workspace-roles";
import { EMPTY_WORKSPACE_ROLES, newRoleId } from "../../lib/workspace-roles";
import { RoleForm } from "./role-form";

export function RolesEditor() {
  const { t } = useTranslation("roles");
  const workspace = useWorkspaceStore((s) => s.current);
  const agents = useAgentStore((s) => s.agents);
  const addToast = useUIStore((s) => s.addToast);
  const { data, isLoading } = useWorkspaceRoles(workspace?.id);
  const save = useSaveWorkspaceRoles(workspace?.id);
  const [draft, setDraft] = useState<WorkspaceRoles>(EMPTY_WORKSPACE_ROLES);

  useEffect(() => {
    if (data) setDraft(data);
  }, [data]);

  const workspaceAgents = useMemo(
    () => agents.map((agent) => agent.name).sort((a, b) => a.localeCompare(b)),
    [agents],
  );

  const dirty = useMemo(() => {
    if (!data) return false;
    return JSON.stringify(draft) !== JSON.stringify(data);
  }, [data, draft]);

  if (!workspace) return null;

  const updateRole = (index: number, next: Role) => {
    const roles = [...draft.roles];
    roles[index] = next;
    setDraft({ ...draft, roles });
  };

  const handleAddRole = () => {
    const id = newRoleId(draft.roles);
    setDraft({
      ...draft,
      roles: [
        ...draft.roles,
        {
          id,
          name: id,
          agents: [],
          provides: [],
          procedures: [],
        },
      ],
    });
  };

  const handleSave = async () => {
    try {
      await save.mutateAsync(draft);
      addToast({ title: t("editor.saved") });
    } catch (err) {
      addToast({
        title: t("editor.saveFailed"),
        description: err instanceof Error ? err.message : String(err),
        variant: "error",
      });
    }
  };

  return (
    <div className="mx-auto max-w-3xl px-8 py-10 space-y-6">
      <header className="space-y-1">
        <h2 className="text-lg font-semibold">{t("editor.title")}</h2>
        <p className="text-sm text-muted-foreground">{t("editor.description")}</p>
      </header>

      {isLoading ? (
        <div className="flex justify-center py-16">
          <Spinner className="h-5 w-5" />
        </div>
      ) : draft.roles.length === 0 ? (
        <Empty className="border-0">
          <EmptyHeader>
            <EmptyTitle>{t("editor.emptyTitle")}</EmptyTitle>
            <EmptyDescription>{t("editor.emptyDescription")}</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button className="rounded-full gap-1.5" onClick={handleAddRole}>
              <Plus className="size-4" />
              {t("editor.addRole")}
            </Button>
          </EmptyContent>
        </Empty>
      ) : (
        <div className="space-y-4">
          {draft.roles.map((role, index) => (
            <RoleForm
              key={`${role.id}-${index}`}
              role={role}
              workspaceAgents={workspaceAgents}
              onChange={(next) => updateRole(index, next)}
              onDelete={() =>
                setDraft({
                  ...draft,
                  roles: draft.roles.filter((_, i) => i !== index),
                })
              }
            />
          ))}
          <Button
            type="button"
            variant="outline"
            className="rounded-full gap-1.5"
            onClick={handleAddRole}
          >
            <Plus className="size-4" />
            {t("editor.addRole")}
          </Button>
        </div>
      )}

      <div className="flex justify-end pt-2">
        <Button
          className="rounded-full"
          disabled={!dirty || save.isPending}
          onClick={() => void handleSave()}
        >
          {save.isPending ? t("editor.saving") : t("editor.save")}
        </Button>
      </div>
    </div>
  );
}
