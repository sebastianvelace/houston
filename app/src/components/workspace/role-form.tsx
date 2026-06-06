import { useTranslation } from "react-i18next";
import { Trash2 } from "lucide-react";
import { Button } from "@houston-ai/core";
import type { DataProvision, Procedure, Role } from "@houston-ai/engine-client";
import { Field, ListSection } from "./role-form-fields";
import { ProcedureRow, ProvisionRow } from "./role-form-rows";

interface RoleFormProps {
  role: Role;
  workspaceAgents: string[];
  onChange: (next: Role) => void;
  onDelete: () => void;
}

export function RoleForm({
  role,
  workspaceAgents,
  onChange,
  onDelete,
}: RoleFormProps) {
  const { t } = useTranslation("roles");

  const toggleAgent = (agentName: string) => {
    const has = role.agents.includes(agentName);
    onChange({
      ...role,
      agents: has
        ? role.agents.filter((name) => name !== agentName)
        : [...role.agents, agentName],
    });
  };

  const updateProvision = (index: number, next: DataProvision) => {
    const provides = [...role.provides];
    provides[index] = next;
    onChange({ ...role, provides });
  };

  const updateProcedure = (index: number, next: Procedure) => {
    const procedures = [...role.procedures];
    procedures[index] = next;
    onChange({ ...role, procedures });
  };

  return (
    <div className="rounded-xl border border-black/5 bg-white p-5 space-y-5">
      <div className="flex items-start justify-between gap-3">
        <div className="grid gap-3 flex-1 sm:grid-cols-2">
          <Field
            label={t("editor.roleId")}
            value={role.id}
            onChange={(id) => onChange({ ...role, id })}
          />
          <Field
            label={t("editor.roleName")}
            value={role.name}
            onChange={(name) => onChange({ ...role, name })}
          />
        </div>
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="rounded-full shrink-0"
          onClick={onDelete}
        >
          <Trash2 className="size-3.5" />
          {t("editor.deleteRole")}
        </Button>
      </div>

      <section className="space-y-2">
        <h4 className="text-sm font-medium">{t("editor.assignedAgents")}</h4>
        {workspaceAgents.length === 0 ? (
          <p className="text-sm text-muted-foreground">{t("editor.noAgents")}</p>
        ) : (
          <div className="flex flex-wrap gap-2">
            {workspaceAgents.map((agentName) => {
              const selected = role.agents.includes(agentName);
              return (
                <button
                  key={agentName}
                  type="button"
                  onClick={() => toggleAgent(agentName)}
                  className={[
                    "rounded-full h-8 px-3 text-sm border transition-colors",
                    selected
                      ? "bg-gray-950 text-white border-gray-950"
                      : "bg-white text-foreground border-black/15 hover:bg-gray-50",
                  ].join(" ")}
                >
                  {agentName}
                </button>
              );
            })}
          </div>
        )}
      </section>

      <ListSection
        title={t("editor.provides")}
        addLabel={t("editor.addProvision")}
        onAdd={() =>
          onChange({
            ...role,
            provides: [
              ...role.provides,
              { id: `info-${role.provides.length + 1}`, description: "" },
            ],
          })
        }
      >
        {role.provides.map((provision, index) => (
          <ProvisionRow
            key={`${provision.id}-${index}`}
            provision={provision}
            removeLabel={t("editor.deleteRole")}
            provisionIdLabel={t("editor.provisionId")}
            descriptionLabel={t("editor.provisionDescription")}
            onChange={(next) => updateProvision(index, next)}
            onRemove={() =>
              onChange({
                ...role,
                provides: role.provides.filter((_, i) => i !== index),
              })
            }
          />
        ))}
      </ListSection>

      <ListSection
        title={t("editor.procedures")}
        addLabel={t("editor.addProcedure")}
        onAdd={() =>
          onChange({
            ...role,
            procedures: [
              ...role.procedures,
              {
                id: `procedure-${role.procedures.length + 1}`,
                description: "",
                requires: [],
              },
            ],
          })
        }
      >
        {role.procedures.map((procedure, index) => (
          <ProcedureRow
            key={`${procedure.id}-${index}`}
            procedure={procedure}
            removeLabel={t("editor.deleteRole")}
            procedureIdLabel={t("editor.procedureId")}
            descriptionLabel={t("editor.procedureDescription")}
            requiresLabel={t("editor.requires")}
            requiresHint={t("editor.requiresHint")}
            requiresPlaceholder={t("editor.requiresPlaceholder")}
            onChange={(next) => updateProcedure(index, next)}
            onRemove={() =>
              onChange({
                ...role,
                procedures: role.procedures.filter((_, i) => i !== index),
              })
            }
          />
        ))}
      </ListSection>
    </div>
  );
}
