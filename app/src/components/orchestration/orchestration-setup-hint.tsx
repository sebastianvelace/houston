import { useTranslation } from "react-i18next";
import { Settings2 } from "lucide-react";
import { Button } from "@houston-ai/core";
import { openRolesSettings } from "../../lib/open-roles-settings";

export type OrchestrationSetupReason = "no_roles" | "unassigned" | "no_procedures";

interface OrchestrationSetupHintProps {
  reason: OrchestrationSetupReason;
}

export function OrchestrationSetupHint({ reason }: OrchestrationSetupHintProps) {
  const { t } = useTranslation("roles");

  return (
    <div className="rounded-xl border border-dashed border-black/10 bg-gray-50/80 p-4 space-y-3">
      <div>
        <h3 className="text-sm font-medium text-foreground">
          {t(`setupHint.${reason}.title`)}
        </h3>
        <p className="text-xs text-muted-foreground mt-1">
          {t(`setupHint.${reason}.description`)}
        </p>
      </div>
      <Button
        type="button"
        variant="outline"
        size="sm"
        className="rounded-full gap-1.5"
        onClick={openRolesSettings}
      >
        <Settings2 className="size-3.5" />
        {t("setupHint.configureRoles")}
      </Button>
    </div>
  );
}
