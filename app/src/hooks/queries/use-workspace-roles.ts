import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { WorkspaceRoles } from "@houston-ai/engine-client";
import { queryKeys } from "../../lib/query-keys";
import { tauriWorkspaces } from "../../lib/tauri";

export function useWorkspaceRoles(workspaceId: string | undefined) {
  return useQuery({
    queryKey: queryKeys.workspaceRoles(workspaceId ?? ""),
    queryFn: () => tauriWorkspaces.getRoles(workspaceId!),
    enabled: !!workspaceId,
  });
}

export function useSaveWorkspaceRoles(workspaceId: string | undefined) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: WorkspaceRoles) =>
      tauriWorkspaces.putRoles(workspaceId!, body),
    onSuccess: (data) => {
      if (workspaceId) {
        qc.setQueryData(queryKeys.workspaceRoles(workspaceId), data);
      }
    },
  });
}
