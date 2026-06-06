import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Crown } from "lucide-react";
import { ChatPanel } from "@houston-ai/chat";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyTitle,
  HoustonAvatar,
  resolveAgentColor,
} from "@houston-ai/core";
import { useAgentStore } from "../../stores/agents";
import { useWorkspaceStore } from "../../stores/workspaces";
import { useFeedStore } from "../../stores/feeds";
import { useUIStore } from "../../stores/ui";
import {
  isActiveSessionStatus,
  useSessionStatus,
} from "../../stores/session-status";
import {
  activeOrchestrationForSession,
  useOrchestrationProgressStore,
} from "../../stores/orchestration-progress";
import { useExecutiveConfig } from "../../hooks/queries/use-executive-config";
import { tauriChat, tauriExecutive } from "../../lib/tauri";
import { useChatDisplayLabels } from "../use-chat-display-labels";
import { OrchestrationProgress } from "../orchestration/orchestration-progress";
import { ConnectedAgentsPanel } from "./connected-agents-panel";

export function ExecutiveManagerView() {
  const { t } = useTranslation(["executive", "shell"]);
  const workspace = useWorkspaceStore((s) => s.current);
  const agents = useAgentStore((s) => s.agents);
  const setCurrentAgent = useAgentStore((s) => s.setCurrent);
  const addToast = useUIStore((s) => s.addToast);
  const pushFeedItem = useFeedStore((s) => s.pushFeedItem);
  const { processLabels, getThinkingMessage } = useChatDisplayLabels();

  const workspaceId = workspace?.id;
  const { data: config } = useExecutiveConfig(workspaceId);
  const executiveAgentName = config?.executiveAgent ?? "Director";

  const director = useMemo(
    () => agents.find((agent) => agent.name === executiveAgentName) ?? null,
    [agents, executiveAgentName],
  );

  useEffect(() => {
    if (director) setCurrentAgent(director);
  }, [director, setCurrentAgent]);

  const agentNames = useMemo(
    () => agents.map((agent) => agent.name).sort((a, b) => a.localeCompare(b)),
    [agents],
  );

  const [sessionKey, setSessionKey] = useState<string | null>(null);
  const directorPath = director?.folderPath ?? "";
  const activeSessionKey = sessionKey ?? "";
  const feedItems = useFeedStore((s) => s.items[directorPath]?.[activeSessionKey]);
  const sessionStatus = useSessionStatus(directorPath, activeSessionKey);
  const isActive = isActiveSessionStatus(sessionStatus);
  const orchestrationRun = useOrchestrationProgressStore((s) =>
    activeSessionKey ? s.runs[activeSessionKey] : undefined,
  );
  const resolvedRun =
    orchestrationRun ?? activeOrchestrationForSession(activeSessionKey);

  const showOnboarding =
    config !== undefined && (config.connectedAgents?.length ?? 0) === 0;

  const handleSend = useCallback(
    async (text: string) => {
      if (!director || !workspaceId || !text.trim()) return;
      const path = director.folderPath;
      const isNewSession = !sessionKey;
      const key = sessionKey ?? crypto.randomUUID();
      if (isNewSession) setSessionKey(key);

      pushFeedItem(path, key, { feed_type: "user_message", data: text });

      try {
        if (isNewSession) {
          const connected = config?.connectedAgents ?? [];
          if (connected.length > 0) {
            useOrchestrationProgressStore.getState().startRun({
              orchestratorPath: path,
              sessionKey: key,
              procedureId: "executive-briefing",
              dataSteps: connected.map((name) => ({ id: name, title: name })),
              procedureTitle: t("executive:progress.synthesis"),
            });
          }
          await tauriExecutive.startBriefing(workspaceId, text, key);
        } else {
          await tauriChat.send(path, text, key);
        }
      } catch (err) {
        addToast({
          title: t("executive:chat.briefingFailed"),
          description: err instanceof Error ? err.message : String(err),
          variant: "error",
        });
      }
    },
    [addToast, config?.connectedAgents, director, pushFeedItem, sessionKey, t, workspaceId],
  );

  const handleStop = useCallback(() => {
    if (!director || !sessionKey) return;
    tauriChat.stop(director.folderPath, sessionKey).catch((err: unknown) => {
      addToast({
        title: t("executive:chat.briefingFailed"),
        description: err instanceof Error ? err.message : String(err),
        variant: "error",
      });
    });
  }, [addToast, director, sessionKey, t]);

  if (!workspaceId) return null;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <header className="flex shrink-0 items-center gap-3 border-b border-black/5 px-6 py-4">
        <div className="flex size-9 items-center justify-center rounded-full bg-gray-950 text-white">
          <Crown className="size-4" />
        </div>
        <div>
          <h1 className="text-lg font-medium text-foreground">
            {t("shell:sidebar.executiveManager")}
          </h1>
          <p className="text-sm text-muted-foreground">
            {director
              ? director.name
              : t("executive:chat.directorMissingTitle")}
          </p>
        </div>
      </header>

      <div className="flex min-h-0 flex-1">
        <ConnectedAgentsPanel
          workspaceId={workspaceId}
          executiveAgentName={executiveAgentName}
          agentNames={agentNames}
        />

        <div className="flex min-h-0 min-w-0 flex-1 flex-col bg-background">
          {!director ? (
            <div className="flex flex-1 items-center justify-center px-6">
              <Empty className="max-w-md border-0">
                <EmptyHeader>
                  <EmptyTitle>{t("executive:chat.directorMissingTitle")}</EmptyTitle>
                  <EmptyDescription>
                    {t("executive:chat.directorMissingDescription")}
                  </EmptyDescription>
                </EmptyHeader>
              </Empty>
            </div>
          ) : (
            <>
              {showOnboarding ? (
                <div className="mx-auto w-full max-w-3xl px-6 pt-6">
                  <div className="rounded-xl border border-black/5 bg-white p-5 shadow-[0_1px_0_rgba(0,0,0,0.05)]">
                    <h2 className="text-base font-medium text-foreground">
                      {t("executive:onboarding.title")}
                    </h2>
                    <p className="mt-1 text-sm text-muted-foreground">
                      {t("executive:onboarding.description")}
                    </p>
                  </div>
                </div>
              ) : null}

              <div className="flex min-h-0 flex-1 flex-col px-2 pt-2">
                <div className="mx-auto flex w-full max-w-3xl min-h-0 flex-1 flex-col">
                  <div className="flex shrink-0 items-center gap-3 px-4 pb-3">
                    <HoustonAvatar
                      color={resolveAgentColor(director.color)}
                      diameter={32}
                      running={isActive}
                    />
                    <p className="text-sm font-medium">{director.name}</p>
                  </div>
                  {resolvedRun ? (
                    <OrchestrationProgress steps={resolvedRun.steps} />
                  ) : null}
                  <div className="min-h-0 flex-1">
                    <ChatPanel
                      sessionKey={activeSessionKey}
                      feedItems={feedItems}
                      onSend={(text) => void handleSend(text)}
                      onStop={isActive ? handleStop : undefined}
                      isLoading={isActive}
                      placeholder={t("executive:chat.placeholder")}
                      processLabels={processLabels}
                      getThinkingMessage={getThinkingMessage}
                    />
                  </div>
                </div>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
