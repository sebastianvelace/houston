import { create } from "zustand";

export type OrchestrationStepStatus = "pending" | "active" | "done";

export interface OrchestrationStep {
  id: string;
  title: string;
  status: OrchestrationStepStatus;
}

interface OrchestrationRun {
  orchestratorPath: string;
  sessionKey: string;
  procedureId: string;
  steps: OrchestrationStep[];
}

interface OrchestrationProgressState {
  runs: Record<string, OrchestrationRun>;
  startRun: (args: {
    orchestratorPath: string;
    sessionKey: string;
    procedureId: string;
    dataSteps: Array<{ id: string; title: string }>;
    procedureTitle: string;
  }) => void;
  markDataStepStarted: (sessionKey: string, providesId: string) => void;
  markDataStepCompleted: (sessionKey: string, providesId: string) => void;
  markProcedureStarted: (sessionKey: string, procedureId: string) => void;
  clearRun: (sessionKey: string) => void;
}

export const useOrchestrationProgressStore = create<OrchestrationProgressState>(
  (set) => ({
    runs: {},
    startRun: ({ orchestratorPath, sessionKey, procedureId, dataSteps, procedureTitle }) =>
      set((state) => ({
        runs: {
          ...state.runs,
          [sessionKey]: {
            orchestratorPath,
            sessionKey,
            procedureId,
            steps: [
              ...dataSteps.map((step) => ({
                id: step.id,
                title: step.title,
                status: "pending" as const,
              })),
              { id: procedureId, title: procedureTitle, status: "pending" as const },
            ],
          },
        },
      })),
    markDataStepStarted: (sessionKey, providesId) =>
      set((state) => {
        const run = state.runs[sessionKey];
        if (!run) return state;
        return {
          runs: {
            ...state.runs,
            [sessionKey]: {
              ...run,
              steps: run.steps.map((step) =>
                step.id === providesId
                  ? { ...step, status: "active" as const }
                  : step.status === "active"
                    ? { ...step, status: "pending" as const }
                    : step,
              ),
            },
          },
        };
      }),
    markDataStepCompleted: (sessionKey, providesId) =>
      set((state) => {
        const run = state.runs[sessionKey];
        if (!run) return state;
        return {
          runs: {
            ...state.runs,
            [sessionKey]: {
              ...run,
              steps: run.steps.map((step) =>
                step.id === providesId ? { ...step, status: "done" as const } : step,
              ),
            },
          },
        };
      }),
    markProcedureStarted: (sessionKey, procedureId) =>
      set((state) => {
        const run = state.runs[sessionKey];
        if (!run) return state;
        return {
          runs: {
            ...state.runs,
            [sessionKey]: {
              ...run,
              steps: run.steps.map((step) =>
                step.id === procedureId
                  ? { ...step, status: "active" as const }
                  : step.status === "active"
                    ? { ...step, status: "done" as const }
                    : step,
              ),
            },
          },
        };
      }),
    clearRun: (sessionKey) =>
      set((state) => {
        const next = { ...state.runs };
        delete next[sessionKey];
        return { runs: next };
      }),
  }),
);

export function activeOrchestrationForSession(
  sessionKey: string | null | undefined,
): OrchestrationRun | null {
  if (!sessionKey) return null;
  return useOrchestrationProgressStore.getState().runs[sessionKey] ?? null;
}

export function activeOrchestrationRun(): OrchestrationRun | null {
  const runs = useOrchestrationProgressStore.getState().runs;
  return (
    Object.values(runs).find((run) =>
      run.steps.some((step) => step.status !== "done"),
    ) ?? null
  );
}
