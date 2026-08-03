export const MOTION_POLICY = {
  page: { trigger: "route or tab content changes", stop: "160ms", durationMs: 160, loops: false, reduced: "opacity only" },
  overlay: { trigger: "dialog or menu opens", stop: "160ms", durationMs: 160, loops: false, reduced: "no translation" },
  drawer: { trigger: "side panel opens", stop: "220ms", durationMs: 220, loops: false, reduced: "no translation" },
  running: { trigger: "backend reports an executing state", stop: "next terminal or waiting refresh", durationMs: 1500, loops: true, reduced: "static marker" },
  pending: { trigger: "operation remains pending for 400ms", stop: "operation resolves", durationMs: 1000, loops: true, reduced: "static icon and text" },
} as const;

export type MotionPolicyName = keyof typeof MOTION_POLICY;
