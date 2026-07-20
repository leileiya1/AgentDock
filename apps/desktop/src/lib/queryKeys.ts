/** Query-key registry (02 §5). Keep keys centralized so event-driven
 * invalidation targets exactly the right cache entries. */
export const qk = {
  env: ["env"] as const,
  onboarding: ["onboarding"] as const,
  providers: ["providers"] as const,
  projects: ["projects"] as const,
  settings: ["settings"] as const,
  storage: ["storage"] as const,
  trash: ["trash"] as const,
  projectSettings: (projectId: string) => ["projectSettings", projectId] as const,
  tasks: (projectId: string) => ["tasks", projectId] as const,
  task: (taskId: string) => ["task", taskId] as const,
  runs: (taskId: string) => ["runs", taskId] as const,
  events: (taskId: string) => ["events", taskId] as const,
  diff: (taskId: string, rev: number) => ["diff", taskId, rev] as const,
  review: (taskId: string, rev: number) => ["review", taskId, rev] as const,
  repair: (taskId: string) => ["repair", taskId] as const,
  governance: (taskId: string, rev: number) => ["governance", taskId, rev] as const,
  executionNodes: ["executionNodes"] as const,
};
