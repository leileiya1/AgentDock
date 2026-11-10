import { expect, test } from "bun:test";
import { cn } from "./utils";

test("semantic font sizes do not erase text colors", () => {
  expect(cn("text-white", "text-meta")).toBe("text-white text-meta");
  expect(cn("text-status-danger", "text-meta")).toBe("text-status-danger text-meta");
  expect(cn("text-meta", "text-body")).toBe("text-body");
});

test("semantic radii and shadows still merge with their own kind", () => {
  expect(cn("rounded-control", "rounded-section")).toBe("rounded-section");
  expect(cn("shadow-press", "shadow-press-action")).toBe("shadow-press-action");
});
