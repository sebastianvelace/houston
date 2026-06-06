import { useTranslation } from "react-i18next";

interface AgentRoleBadgesProps {
  roleName: string | null;
  isProvider: boolean;
  isOrchestrator: boolean;
}

export function AgentRoleBadges({
  roleName,
  isProvider,
  isOrchestrator,
}: AgentRoleBadgesProps) {
  const { t } = useTranslation("roles");

  if (!roleName && !isProvider && !isOrchestrator) return null;

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
