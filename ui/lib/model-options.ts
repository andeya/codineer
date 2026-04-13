import type { ModelGroupData } from "@/lib/tauri";

export interface ModelSelectOption {
  value: string;
  label: string;
}

/** Flatten `list_model_groups` payload into select options (`provider/model`).
 *  When `onlyAvailable` is true, groups with `available === false` are skipped. */
export function modelGroupsToSelectOptions(
  groups: ModelGroupData[],
  onlyAvailable = false,
): ModelSelectOption[] {
  const opts: ModelSelectOption[] = [];
  for (const g of groups) {
    if (onlyAvailable && !g.available) continue;
    for (const m of g.models) {
      const id = `${g.provider}/${m}`;
      opts.push({ value: id, label: id });
    }
  }
  return opts;
}

/** Prepend current setting if it is not in the catalog (custom / stale id). */
export function withCurrentModelOption(
  options: ModelSelectOption[],
  current: string | undefined | null,
): ModelSelectOption[] {
  const c = current?.trim() ?? "";
  if (c && !options.some((o) => o.value === c)) {
    return [{ value: c, label: c }, ...options];
  }
  return options;
}

/** Short name for status bar chip (`anthropic/foo` → `foo`). */
export function shortModelDisplay(model: string | undefined | null): string {
  if (!model?.trim()) return "";
  return model.includes("/") ? (model.split("/").pop() ?? model) : model;
}
