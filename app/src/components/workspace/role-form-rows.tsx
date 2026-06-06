import { Trash2 } from "lucide-react";
import { Button } from "@houston-ai/core";
import type { DataProvision, Procedure } from "@houston-ai/engine-client";
import { Field, formatRequires, parseRequires, TextArea } from "./role-form-fields";

export function ProvisionRow({
  provision,
  provisionIdLabel,
  descriptionLabel,
  removeLabel,
  onChange,
  onRemove,
}: {
  provision: DataProvision;
  provisionIdLabel: string;
  descriptionLabel: string;
  removeLabel: string;
  onChange: (next: DataProvision) => void;
  onRemove: () => void;
}) {
  return (
    <div className="space-y-2 rounded-lg bg-secondary/40 p-3">
      <Field
        label={provisionIdLabel}
        value={provision.id}
        onChange={(id) => onChange({ ...provision, id })}
      />
      <TextArea
        label={descriptionLabel}
        value={provision.description}
        onChange={(description) => onChange({ ...provision, description })}
      />
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className="rounded-full"
        onClick={onRemove}
      >
        <Trash2 className="size-3.5" />
        {removeLabel}
      </Button>
    </div>
  );
}

export function ProcedureRow({
  procedure,
  procedureIdLabel,
  descriptionLabel,
  requiresLabel,
  requiresHint,
  requiresPlaceholder,
  removeLabel,
  onChange,
  onRemove,
}: {
  procedure: Procedure;
  procedureIdLabel: string;
  descriptionLabel: string;
  requiresLabel: string;
  requiresHint: string;
  requiresPlaceholder: string;
  removeLabel: string;
  onChange: (next: Procedure) => void;
  onRemove: () => void;
}) {
  return (
    <div className="space-y-2 rounded-lg bg-secondary/40 p-3">
      <Field
        label={procedureIdLabel}
        value={procedure.id}
        onChange={(id) => onChange({ ...procedure, id })}
      />
      <TextArea
        label={descriptionLabel}
        value={procedure.description}
        onChange={(description) => onChange({ ...procedure, description })}
      />
      <TextArea
        label={requiresLabel}
        hint={requiresHint}
        placeholder={requiresPlaceholder}
        value={formatRequires(procedure.requires)}
        onChange={(value) =>
          onChange({ ...procedure, requires: parseRequires(value) })
        }
      />
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className="rounded-full"
        onClick={onRemove}
      >
        <Trash2 className="size-3.5" />
        {removeLabel}
      </Button>
    </div>
  );
}
