# AgentFlow UI primitives

These primitives keep page structure predictable. They do not own product data or backend calls.

| Primitive | Use for | Do not use for | Size and behavior |
| --- | --- | --- | --- |
| `PageHeader` | One page identity and its single primary action | Promotional hero copy or repeated task explanations | 64 px minimum; title is `h1`; actions remain keyboard reachable |
| `SectionHeader` | A named region with optional count/action | Wrapping every small card | 40 px minimum; title is `h2` |
| `Toolbar` | Search, filter and view controls | Status explanations | 40 px minimum; `role=toolbar`; every icon action needs a name |
| `StatusBadge` | Persisted workflow state | Provider identity or button labels | Small text plus shape/color; never color-only |
| `ProviderIcon` / `AgentMark` | Provider identity in rows and tracks | Large colored provider cards | 22–40 px; brand color stays inside the mark |
| `DataRow` | Read-only key result or fact | A decision with a primary action | 40 px minimum; value remains selectable |
| `ActionRow` | One item and at most one direct action | Multiple competing primary actions | 40 px minimum; the whole row is not clickable if it contains buttons |
| `EmptyState` | A factual absence and one next step | Celebration, illustration or marketing copy | One heading, one hint, zero or one action |
| `InlineNotice` | Local status, caution or recoverable error | Replacing the whole page after a local failure | Uses `status` or `alert`; never hides completed content |
| `Inspector` | Details for the selected execution node | Primary workflow content | Side panel/drawer; labelled landmark; independent scroll |
| `Dialog` / `SidePanel` | Blocking decision / drill-down sheet | Routine inline content | Focus trap, Escape, focus restore; footer stays visible |
| `Tabs` / `SegmentedControl` | Mutually exclusive peer views | Sequential workflow stages | Arrow/tab semantics belong to the concrete tab implementation |

Density has two modes: comfortable is the default; compact only changes row height and spacing in task lists, execution tracks and logs. It must never hide fields or change status color.

All new page code uses semantic color, radius, typography and motion utilities from `styles/theme.css`. Literal colors and ad-hoc pixel typography are CI failures.
