export type AttentionNavResult =
  | { type: "focus"; index: number }
  | { type: "open"; index: number }
  | null;

export function attentionNav(key: string, currentIndex: number, itemCount: number): AttentionNavResult {
  if (itemCount <= 0) return null;
  const index = Math.min(Math.max(currentIndex, 0), itemCount - 1);

  switch (key) {
    case "ArrowDown":
      return { type: "focus", index: Math.min(index + 1, itemCount - 1) };
    case "ArrowUp":
      return { type: "focus", index: Math.max(index - 1, 0) };
    case "Home":
      return { type: "focus", index: 0 };
    case "End":
      return { type: "focus", index: itemCount - 1 };
    case "Enter":
    case " ":
      return { type: "open", index };
    default:
      return null;
  }
}
