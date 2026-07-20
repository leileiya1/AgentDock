import { useNavigate } from "react-router-dom";
import { open } from "@tauri-apps/plugin-dialog";
import { useCompleteOnboarding, useOnboarding } from "@/hooks/useEnv";
import { useImportProject } from "@/hooks/useProjects";
import { ProviderCatalog } from "@/components/ProviderCatalog";
import { EmptyState } from "@/components/EmptyState";
import { ErrorState } from "@/components/ErrorState";
import { Skeleton } from "@/components/Skeleton";
import { Button } from "@/components/ui/button";
import { errorLine, toAppError } from "@/copy/errors";
import { toast } from "@/stores/toastStore";

const sectionH = "mb-3 text-[13px] font-semibold uppercase tracking-wider text-t2";

export function Onboarding() {
  const onboarding = useOnboarding();
  const importProject = useImportProject();
  const complete = useCompleteOnboarding();
  const navigate = useNavigate();

  const pickAndImport = async () => {
    try {
      const dir = await open({ directory: true, multiple: false });
      if (typeof dir !== "string") return;
      const project = await importProject.mutateAsync(dir);
      toast.info("项目已导入");
      navigate(`/p/${project.id}`);
    } catch (error) {
      const display = toAppError(error);
      toast.error(display.detail ? `${display.title} · ${display.detail}` : errorLine(error));
    }
  };

  const finish = async () => {
    try {
      await complete.mutateAsync();
    } catch {
      /* Completion only suppresses this guide; entering the app remains safe. */
    }
    navigate("/");
  };

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header className="flex shrink-0 items-center justify-between border-b border-line/70 px-6 py-4">
        <h1 className="text-xl font-semibold tracking-tight">欢迎使用 AgentFlow</h1>
        <Button variant="outline" onClick={() => onboarding.refetch()} disabled={onboarding.isFetching}>
          {onboarding.isFetching ? "检测中…" : "重新检测"}
        </Button>
      </header>

      <div className="flex-1 overflow-y-auto px-6 py-5">
        <div className="mx-auto max-w-3xl">
          <p className="mb-4 text-[13px] text-t2">
            连接至少两个 Provider，即可分别负责开发和独立审查。未安装的 CLI 可以稍后直接安装。
          </p>

          {onboarding.isLoading ? (
            <div className="flex flex-col gap-3">{Array.from({ length: 6 }).map((_, index) => <Skeleton key={index} height={56} />)}</div>
          ) : onboarding.isError ? (
            <ErrorState error={onboarding.error} onRetry={() => onboarding.refetch()} />
          ) : onboarding.data && (
            <>
              {onboarding.data.notices.length > 0 && (
                <ul className="mb-4 list-inside list-disc rounded-[var(--radius-panel)] border border-bad/25 bg-panel px-4 py-3 text-[13px] text-t2">
                  {onboarding.data.notices.map((notice, index) => <li key={index}>{notice}</li>)}
                </ul>
              )}

              <section className="mb-6">
                <h2 className={sectionH}>AI Provider</h2>
                <div className="flex flex-col gap-4 rounded-[var(--radius-panel)] border border-line bg-panel/60 p-4">
                  <ProviderCatalog env={onboarding.data.env} />
                </div>
              </section>
            </>
          )}

          <section className="mb-6">
            <h2 className={sectionH}>导入项目</h2>
            <div className="rounded-[var(--radius-panel)] border border-line bg-panel/60 p-2">
              <EmptyState
                title="导入你的第一个项目"
                hint="选择一个已经是 Git 仓库的目录。非 Git 目录会给出明确指引。"
                action={
                  <Button variant="primary" onClick={pickAndImport} disabled={importProject.isPending}>
                    {importProject.isPending ? "导入中…" : "选择目录并导入"}
                  </Button>
                }
              />
            </div>
          </section>

          <div className="flex justify-end border-t border-line/70 pt-4">
            <Button variant="primary" onClick={finish}>完成引导</Button>
          </div>
        </div>
      </div>
    </div>
  );
}
