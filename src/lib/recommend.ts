import type { ModelLibraryEntry } from "../types/models";

/** Pick one starter model that fits this machine (smallest that "runs well"). */
export function pickRecommendedModel(
  entries: ModelLibraryEntry[],
): ModelLibraryEntry | null {
  if (entries.length === 0) return null;
  const well = entries.filter((e) => e.fit === "runsWell");
  const pool =
    well.length > 0
      ? well
      : entries.filter((e) => e.fit === "mightBeSlow");
  const candidates = pool.length > 0 ? pool : entries;
  return [...candidates].sort((a, b) => {
    if (a.model.paramsB !== b.model.paramsB) {
      return a.model.paramsB - b.model.paramsB;
    }
    return a.model.sizeBytes - b.model.sizeBytes;
  })[0];
}
