export const MAX_REVISIONS_MIN = 1;
export const MAX_REVISIONS_MAX = 20;

export function maxRevisionsError(value: string): string | null {
  const normalized = value.trim();
  if (!normalized) return "请输入最大返工轮数（1–20）";
  if (!/^\d+$/.test(normalized)) return "最大返工轮数必须是 1–20 的整数";
  const parsed = Number(normalized);
  if (!Number.isSafeInteger(parsed) || parsed < MAX_REVISIONS_MIN || parsed > MAX_REVISIONS_MAX) {
    return "最大返工轮数必须在 1–20 之间";
  }
  return null;
}

export function parseMaxRevisions(value: string): number | null {
  return maxRevisionsError(value) ? null : Number(value.trim());
}
