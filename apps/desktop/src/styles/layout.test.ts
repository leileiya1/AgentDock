import { describe, expect, it } from "bun:test";

const THEME = await Bun.file(new URL("./theme.css", import.meta.url)).text();

/**
 * The route panels own their scrolling. Keeping the document itself fixed prevents
 * scroll chaining from moving the whole application shell and revealing blank space.
 */
describe("application viewport containment", () => {
  it("locks both document roots instead of using body as a scroll container", () => {
    expect(THEME).toMatch(/html\s*\{[^}]*overflow:\s*hidden;/s);
    expect(THEME).toMatch(/body\s*\{[^}]*overflow:\s*hidden;/s);
    expect(THEME).not.toMatch(/body\s*\{[^}]*overflow:\s*auto;/s);
  });

  it("keeps the React root bounded to the viewport", () => {
    expect(THEME).toMatch(/#root\s*\{[^}]*min-height:\s*0;[^}]*overflow:\s*hidden;/s);
  });
});
