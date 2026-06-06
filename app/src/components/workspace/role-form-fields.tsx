import type { ReactNode } from "react";
import { Plus } from "lucide-react";
import { Button } from "@houston-ai/core";

export function ListSection({
  title,
  addLabel,
  onAdd,
  children,
}: {
  title: string;
  addLabel: string;
  onAdd: () => void;
  children: ReactNode;
}) {
  return (
    <section className="space-y-3">
      <div className="flex items-center justify-between gap-3">
        <h4 className="text-sm font-medium">{title}</h4>
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="rounded-full gap-1.5"
          onClick={onAdd}
        >
          <Plus className="size-3.5" />
          {addLabel}
        </Button>
      </div>
      <div className="space-y-2">{children}</div>
    </section>
  );
}

export function Field({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="block space-y-1.5">
      <span className="text-xs text-muted-foreground">{label}</span>
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full rounded-md border border-border bg-card px-3 py-2 text-sm outline-none focus:ring-1 focus:ring-ring"
      />
    </label>
  );
}

export function TextArea({
  label,
  value,
  onChange,
  hint,
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  hint?: string;
  placeholder?: string;
}) {
  return (
    <label className="block space-y-1.5">
      <span className="text-xs text-muted-foreground">{label}</span>
      {hint ? <span className="block text-xs text-muted-foreground">{hint}</span> : null}
      <textarea
        value={value}
        placeholder={placeholder}
        rows={3}
        onChange={(e) => onChange(e.target.value)}
        className="w-full rounded-md border border-border bg-card px-3 py-2 text-sm outline-none focus:ring-1 focus:ring-ring resize-y min-h-[72px]"
      />
    </label>
  );
}

export function parseRequires(value: string): string[] {
  return value
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
}

export function formatRequires(requires: string[]): string {
  return requires.join("\n");
}
