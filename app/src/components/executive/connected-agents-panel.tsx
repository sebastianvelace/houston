import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, Users } from "lucide-react";
import { Button, Empty, EmptyDescription, EmptyHeader, EmptyTitle, Spinner, cn } from "@houston-ai/core";
import type { ExecutiveConfig } from "@houston-ai/engine-client";
import { useUIStore } from "../../stores/ui";
import {
  useExecutiveConfig,
  useSaveExecutiveConfig,
} from "../../hooks/queries/use-executive-config";

interface ConnectedAgentsPanelProps {
  workspaceId: string;
  executiveAgentName: string;
  agentNames: string[];
}

export function ConnectedAgentsPanel({
  workspaceId,
  executiveAgentName,
  agentNames,
}: ConnectedAgentsPanelProps) {
  const { t } = useTranslation("executive");
  const addToast = useUIStore((s) => s.addToast);
  const { data, isLoading, isError, refetch, isFetching } =
    useExecutiveConfig(workspaceId);
  const save = useSaveExecutiveConfig(workspaceId);
  const [draft, setDraft] = useState<ExecutiveConfig | null>(null);

  useEffect(() => {
    if (data) setDraft(data);
  }, [data]);

  const selectableAgents = useMemo(
    () => agentNames.filter((name) => name !== executiveAgentName),
    [agentNames, executiveAgentName],
  );

  const dirty = useMemo(() => {
    if (!data || !draft) return false;
    return JSON.stringify(draft.connectedAgents) !== JSON.stringify(data.connectedAgents);
  }, [data, draft]);

  const toggleAgent = (agentName: string) => {
    if (!draft) return;
    const has = draft.connectedAgents.includes(agentName);
    setDraft({
      ...draft,
      connectedAgents: has
        ? draft.connectedAgents.filter((name) => name !== agentName)
        : [...draft.connectedAgents, agentName],
    });
  };

  const handleSave = async () => {
    if (!draft) return;
    try {
      await save.mutateAsync(draft);
      addToast({ title: t("connectedAgents.saved") });
    } catch (err) {
      addToast({
        title: t("connectedAgents.saveFailed"),
        description: err instanceof Error ? err.message : String(err),
        variant: "error",
      });
    }
  };

  return (
    <aside className="flex h-full w-[300px] shrink-0 flex-col border-r border-black/5 bg-[#f9f9f9]">
      <div className="border-b border-black/5 px-5 py-4">
        <div className="flex items-center gap-2">
          <div className="flex size-8 items-center justify-center rounded-full bg-white shadow-[0_1px_0_rgba(0,0,0,0.05)]">
            <Users className="size-4 text-foreground/70" />
          </div>
          <div>
            <h2 className="text-sm font-medium text-foreground">
              {t("connectedAgents.title")}
            </h2>
            <p className="text-xs text-muted-foreground mt-0.5">
              {t("connectedAgents.description")}
            </p>
          </div>
        </div>
      </div>

      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto px-5 py-4">
        {isError ? (
          <Empty className="border-0 bg-transparent py-8">
            <EmptyHeader>
              <EmptyTitle>{t("connectedAgents.loadFailedTitle")}</EmptyTitle>
              <EmptyDescription>{t("connectedAgents.loadFailedDescription")}</EmptyDescription>
            </EmptyHeader>
            <Button
              className="mt-4 rounded-full"
              variant="secondary"
              disabled={isFetching}
              onClick={() => void refetch()}
            >
              {isFetching ? t("connectedAgents.loading") : t("connectedAgents.retry")}
            </Button>
          </Empty>
        ) : isLoading || !draft ? (
          <div className="flex flex-1 items-center justify-center">
            <Spinner className="size-5 text-muted-foreground" />
          </div>
        ) : selectableAgents.length === 0 ? (
          <Empty className="border-0 bg-transparent py-8">
            <EmptyHeader>
              <EmptyTitle>{t("connectedAgents.emptyTitle")}</EmptyTitle>
              <EmptyDescription>{t("connectedAgents.emptyDescription")}</EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : (
          <div className="space-y-2">
            {selectableAgents.map((agentName) => {
              const selected = draft.connectedAgents.includes(agentName);
              return (
                <button
                  key={agentName}
                  type="button"
                  onClick={() => toggleAgent(agentName)}
                  className={cn(
                    "flex w-full items-center gap-3 rounded-xl border px-3 py-2.5 text-left text-sm transition-colors",
                    selected
                      ? "border-gray-950/20 bg-white shadow-[0_1px_0_rgba(0,0,0,0.04)]"
                      : "border-black/5 bg-white/60 hover:bg-white",
                  )}
                >
                  <span
                    className={cn(
                      "flex size-5 shrink-0 items-center justify-center rounded-full border",
                      selected
                        ? "border-gray-950 bg-gray-950 text-white"
                        : "border-black/15 bg-white",
                    )}
                  >
                    {selected ? <Check className="size-3" strokeWidth={3} /> : null}
                  </span>
                  <span className="truncate font-medium">{agentName}</span>
                </button>
              );
            })}
          </div>
        )}
      </div>

      {draft && selectableAgents.length > 0 ? (
        <div className="border-t border-black/5 px-5 py-4">
          <Button
            className="w-full rounded-full"
            disabled={!dirty || save.isPending}
            onClick={() => void handleSave()}
          >
            {save.isPending ? t("connectedAgents.saving") : t("connectedAgents.save")}
          </Button>
        </div>
      ) : null}
    </aside>
  );
}
