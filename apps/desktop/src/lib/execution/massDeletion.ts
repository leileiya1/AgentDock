// §45: a single round that deletes at least this many files is treated as a high-risk mass
// deletion and warned about before a merge is approved. One source of truth for every surface.
export const MASS_DELETE_THRESHOLD = 10;

/** Whether a revision's deleted-file count crosses the mass-deletion warning threshold (§45). */
export function isMassDeletion(deletedFiles: number | null | undefined): boolean {
  return (deletedFiles ?? 0) >= MASS_DELETE_THRESHOLD;
}
