import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Play } from "lucide-react";
import { Button } from "@houston-ai/core";
import type { Procedure } from "@houston-ai/engine-client";

interface OrchestratorProceduresProps {
  procedures: Procedure[];
  onExecute: (procedureId: string) => Promise<void>;
}

export function OrchestratorProcedures({
  procedures,
  onExecute,
}: OrchestratorProceduresProps) {
  const { t } = useTranslation("roles");
  const [runningId, setRunningId] = useState<string | null>(null);

  if (procedures.length === 0) return null;

  return (
    <div className="rounded-xl border border-black/5 bg-white p-4 space-y-3">
      <div>
        <h3 className="text-sm font-medium text-foreground">
          {t("procedures.title")}
        </h3>
        <p className="text-xs text-muted-foreground mt-1">
          {t("procedures.description")}
        </p>
      </div>
      <div className="space-y-2">
        {procedures.map((procedure) => (
          <div
            key={procedure.id}
            className="flex items-start justify-between gap-3 rounded-lg border border-black/5 px-3 py-2.5"
          >
            <div className="min-w-0">
              <p className="text-sm font-medium text-foreground truncate">
                {procedure.id}
              </p>
              <p className="text-xs text-muted-foreground mt-0.5">
                {procedure.description}
              </p>
              {procedure.requires.length > 0 && (
                <p className="text-xs text-muted-foreground mt-1">
                  {t("procedures.requiresCount", {
                    count: procedure.requires.length,
                  })}
                </p>
              )}
            </div>
            <Button
              size="sm"
              className="rounded-full shrink-0 gap-1.5"
              disabled={runningId !== null}
              onClick={() => {
                setRunningId(procedure.id);
                void onExecute(procedure.id).finally(() => setRunningId(null));
              }}
            >
              <Play className="size-3 fill-current" />
              {runningId === procedure.id
                ? t("procedures.running")
                : t("procedures.execute")}
            </Button>
          </div>
        ))}
      </div>
    </div>
  );
}
