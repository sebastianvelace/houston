import { useTranslation } from "react-i18next";
import type { OrchestrationSetupReason } from "../orchestration/orchestration-setup-hint";
import { openRolesSettings } from "../../lib/open-roles-settings";

interface AgentRoleBadgesProps {
  roleName: string | null;
  isProvider: boolean;
  isOrchestrator: boolean;
  setupHint?: OrchestrationSetupReason | null;
}

export function AgentRoleBadges({
  roleName,
  isProvider,
  isOrchestrator,
  setupHint = null,
}: AgentRoleBadgesProps) {
  const { t } = useTranslation("roles");

  if (!roleName && !isProvider && !isOrchestrator) {
    if (!setupHint) return null;
    return (
      <button
        type="button"
        onClick={(event) => {
          event.stopPropagation();
          openRolesSettings();
        }}
        className="rounded-full bg-gray-100 px-2 py-0.5 text-[10px] text-muted-foreground hover:text-foreground hover:bg-gray-200 transition-colors truncate max-w-[88px]"
        title={t(`setupHint.${setupHint}.title`)}
      >
        {t("setupHint.sidebarLink")}
      </button>
    );
  }

  return (
    <div className="flex flex-wrap items-center gap-1 max-w-[88px] justify-end">
      {roleName ? (
        <span className="rounded-full bg-gray-100 px-2 py-0.5 text-[10px] text-foreground truncate max-w-full">
          {roleName}
        </span>
      ) : null}
      {isProvider ? (
        <span className="rounded-full bg-gray-100 px-2 py-0.5 text-[10px] text-muted-foreground">
          {t("badges.provider")}
        </span>
      ) : null}
      {isOrchestrator ? (
        <span className="rounded-full bg-gray-100 px-2 py-0.5 text-[10px] text-muted-foreground">
          {t("badges.orchestrator")}
        </span>
      ) : null}
    </div>
  );
}
