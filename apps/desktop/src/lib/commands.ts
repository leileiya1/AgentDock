import type { AppError } from "@/generated/bindings";

type Result<T> = { status: "ok"; data: T } | { status: "error"; error: AppError };

/**
 * The only adaptor over the generated bindings' Result shape: unwrap `ok` or
 * throw the `AppError` so TanStack Query / try-catch see a rejection. All
 * backend access still flows through `commands` from generated/bindings.ts.
 */
export async function unwrap<T>(p: Promise<Result<T>>): Promise<T> {
  const r = await p;
  if (r.status === "ok") return r.data;
  throw r.error;
}
