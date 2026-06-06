import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { ExecutiveConfig } from "@houston-ai/engine-client";
import { queryKeys } from "../../lib/query-keys";
import { tauriExecutive } from "../../lib/tauri";

export function useExecutiveConfig(workspaceId: string | undefined) {
  return useQuery({
    queryKey: queryKeys.executiveConfig(workspaceId ?? ""),
    queryFn: () => tauriExecutive.getConfig(workspaceId!),
    enabled: !!workspaceId,
  });
}

export function useSaveExecutiveConfig(workspaceId: string | undefined) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: ExecutiveConfig) =>
      tauriExecutive.putConfig(workspaceId!, body),
    onSuccess: (data) => {
      if (workspaceId) {
        qc.setQueryData(queryKeys.executiveConfig(workspaceId), data);
      }
    },
  });
}
