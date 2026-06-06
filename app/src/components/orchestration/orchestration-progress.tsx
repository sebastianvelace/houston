import { useTranslation } from "react-i18next";
import { Check, Circle, Loader2 } from "lucide-react";
import { cn } from "@houston-ai/core";
import type { OrchestrationStep } from "../../stores/orchestration-progress";

interface OrchestrationProgressProps {
  steps: OrchestrationStep[];
}

export function OrchestrationProgress({ steps }: OrchestrationProgressProps) {
  const { t } = useTranslation("roles");
  const doneCount = steps.filter((step) => step.status === "done").length;

  if (steps.length === 0) return null;

  return (
    <div className="mx-auto max-w-3xl w-full px-6 pb-4">
      <div className="rounded-xl border border-black/5 bg-white p-4 shadow-[0_1px_0_rgba(0,0,0,0.05)]">
        <div className="mb-3">
          <h3 className="text-sm font-medium text-foreground">
            {t("progress.title")}
          </h3>
          <p className="text-xs text-muted-foreground mt-0.5">
            {t("progress.stepsComplete", {
              done: doneCount,
              total: steps.length,
            })}
          </p>
        </div>
        <div className="space-y-1">
          {steps.map((step) => (
            <StepRow key={step.id} step={step} />
          ))}
        </div>
      </div>
    </div>
  );
}

function StepRow({ step }: { step: OrchestrationStep }) {
  return (
    <div
      className={cn(
        "flex items-start gap-3 px-3 py-2 rounded-lg",
        step.status === "active" && "bg-accent/50",
      )}
    >
      <div className="mt-0.5 shrink-0">
        {step.status === "done" && (
          <div className="size-5 rounded-full bg-[#00a240] flex items-center justify-center">
            <Check className="size-3 text-white" strokeWidth={3} />
          </div>
        )}
        {step.status === "active" && (
          <Loader2
            className="size-5 text-foreground/60 animate-spin"
            strokeWidth={1.5}
          />
        )}
        {step.status === "pending" && (
          <Circle className="size-5 text-muted-foreground/25" strokeWidth={1.5} />
        )}
      </div>
      <p
        className={cn(
          "text-sm leading-snug",
          step.status === "done" && "text-foreground/50",
          step.status === "active" && "text-foreground font-medium",
          step.status === "pending" && "text-foreground/60",
        )}
      >
        {step.title}
      </p>
    </div>
  );
}
