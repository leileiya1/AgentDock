import { clsx, type ClassValue } from "clsx";
import { extendTailwindMerge } from "tailwind-merge";

/**
 * tailwind-merge must know our semantic theme scales. Without this, `text-meta`
 * (a font size) is mistaken for a text color and silently drops `text-white`
 * from the same element, e.g. small primary buttons rendered dark-on-navy.
 */
const twMerge = extendTailwindMerge({
  extend: {
    theme: {
      text: ["micro", "meta", "body", "section", "page"],
      radius: ["row", "control", "section", "card", "overlay", "pill", "circle"],
      shadow: ["float", "press", "press-action", "press-danger"],
    },
  },
});

/** shadcn-convention class combiner: clsx + tailwind-merge. */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}
