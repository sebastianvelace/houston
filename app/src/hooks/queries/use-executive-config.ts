import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { ExecutiveConfig } from "@houston-ai/engine-client";
import { queryKeys } from "../../lib/query-keys";
import { tauriExecutive } from "../../lib/tauri";

function normalizeExecutiveConfig(config: ExecutiveConfig): ExecutiveConfig {
  return {
    version: config.version ?? 1,
    executiveAgent: config.executiveAgent?.trim() || "Director",
    connectedAgents: config.connectedAgents ?? [],
  };
}

export function useExecutiveConfig(workspaceId: string | undefined) {
  return useQuery({
    queryKey: queryKeys.executiveConfig(workspaceId ?? ""),
    queryFn: async () =>
      normalizeExecutiveConfig(await tauriExecutive.getConfig(workspaceId!)),
    enabled: !!workspaceId,
  });
}

export function useSaveExecutiveConfig(workspaceId: string | undefined) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: ExecutiveConfig) =>
      tauriExecutive.putConfig(workspaceId!, normalizeExecutiveConfig(body)),
    onSuccess: (data) => {
      if (workspaceId) {
        qc.setQueryData(
          queryKeys.executiveConfig(workspaceId),
          normalizeExecutiveConfig(data),
        );
      }
    },
  });
}
