import type { HoustonClient } from "./client";
import type {
  ExecutiveBriefingRequest,
  ExecutiveBriefingResponse,
  ExecutiveConfig,
} from "./types";

export type {
  ExecutiveBriefingRequest,
  ExecutiveBriefingResponse,
  ExecutiveConfig,
} from "./types";

export function getExecutiveConfig(
  client: HoustonClient,
  workspaceId: string,
): Promise<ExecutiveConfig> {
  return client.getExecutiveConfig(workspaceId);
}

export function putExecutiveConfig(
  client: HoustonClient,
  workspaceId: string,
  config: ExecutiveConfig,
): Promise<ExecutiveConfig> {
  return client.putExecutiveConfig(workspaceId, config);
}

export function postExecutiveBriefing(
  client: HoustonClient,
  workspaceId: string,
  body: ExecutiveBriefingRequest,
): Promise<ExecutiveBriefingResponse> {
  return client.postExecutiveBriefing(workspaceId, body);
}
