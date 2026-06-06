import { useUIStore } from "../stores/ui";

export function openRolesSettings() {
  useUIStore.getState().setSettingsSection("roles");
  useUIStore.getState().setViewMode("settings");
}
