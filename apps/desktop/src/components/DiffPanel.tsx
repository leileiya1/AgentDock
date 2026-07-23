import { Suspense, lazy, useEffect, useMemo, useState } from "react";
import { FlaskConical, Lock, ShieldAlert, TriangleAlert, type LucideIcon } from "lucide-react";
import type { DiffPayload, FileDiff } from "@/generated/bindings";
import { RISK_META, isTestFile, summarizeRisk, type RiskKind } from "@/lib/risk";
import { cn } from "@/lib/utils";
import { CopyText } from "./CopyText";
import { EmptyState } from "./EmptyState";
import { Skeleton } from "./Skeleton";

const MonacoDiff = lazy(() => import("./MonacoDiff"));

interface Props {
  diff: DiffPayload;
  jumpFile?: string | null;
  jumpLine?: number | null;
}

const RISK_ICON: Record<RiskKind, LucideIcon> = {
  control_plane: ShieldAlert,
  test_removed: FlaskConical,
  permission_config: Lock,
  security_sensitive: TriangleAlert,
};

type FileFilter = "all" | "risk" | "tests";

/**
 * Diff 页 (05 §7). 文件树独立标记控制面文件、测试删除、权限/配置和安全敏感文件，
 * 并支持只看风险或只看测试——风险不再淹没在一长串文件里 (05 §6.9)。
 */
export function DiffPanel({ diff, jumpFile, jumpLine }: Props) {
  const files = diff.files;
  const risk = useMemo(() => summarizeRisk(files), [files]);
  const [filter, setFilter] = useState<FileFilter>("all");
  const [selected, setSelected] = useState<string | null>(files[0]?.path ?? null);

  const visible = useMemo(() => {
    if (filter === "risk") return files.filter((f) => risk.byFile.has(f.path));
    if (filter === "tests") return files.filter((f) => isTestFile(f.path));
    return files;
  }, [files, filter, risk]);

  useEffect(() => {
    if (jumpFile && files.some((f) => f.path === jumpFile)) {
      // 跳转的文件可能被当前筛选隐藏，先回到全部再选中。
      setFilter("all");
      setSelected(jumpFile);
    }
  }, [jumpFile, files]);

  const current = useMemo(() => files.find((f) => f.path === selected) ?? null, [files, selected]);
  const currentRisks = current ? risk.byFile.get(current.path) ?? [] : [];

  if (files.length === 0) {
    return <EmptyState title="这一轮没有可显示的改动" hint="可能是空改动，或全部文件都被排除规则过滤了。" />;
  }

  return (
    <div className="flex h-full min-h-0">
      <div className="flex w-52 shrink-0 flex-col border-r border-line/70 lg:w-60 wide:w-72">
        <div className="flex shrink-0 items-center justify-between gap-2 border-b border-line/70 px-3 py-2 text-[12px] text-t3">
          <span>{files.length} 个文件</span>
          {diff.truncated && (
            <span className="text-human" title="diff 过大，部分文件内容已省略">已截断</span>
          )}
        </div>

        <div className="flex shrink-0 gap-1 border-b border-line/70 px-2 py-1.5" role="tablist" aria-label="文件筛选">
          <FilterTab id="all" active={filter} onClick={setFilter} label={`全部 ${files.length}`} />
          <FilterTab
            id="risk"
            active={filter}
            onClick={setFilter}
            label={`风险 ${risk.byFile.size}`}
            disabled={risk.byFile.size === 0}
            tone={risk.hasHighRisk ? "attention" : undefined}
          />
          <FilterTab
            id="tests"
            active={filter}
            onClick={setFilter}
            label={`测试 ${risk.touchedTests.length + risk.removedTests.length}`}
            disabled={risk.touchedTests.length + risk.removedTests.length === 0}
          />
        </div>

        {risk.removedTests.length > 0 && (
          <div className="shrink-0 border-b border-human/40 bg-human-bg px-3 py-2 text-[12px] text-t1">
            本轮删除了 {risk.removedTests.length} 个测试文件，测试覆盖可能下降。
          </div>
        )}

        <ul className="flex-1 list-none overflow-y-auto p-1">
          {visible.map((f) => (
            <FileRow
              key={f.path}
              file={f}
              risks={risk.byFile.get(f.path) ?? []}
              active={f.path === selected}
              onClick={() => setSelected(f.path)}
            />
          ))}
        </ul>
      </div>

      <div className="flex min-w-0 flex-1 flex-col">
        {currentRisks.length > 0 && (
          <div className="shrink-0 border-b border-human bg-human-bg px-3 py-2">
            {currentRisks.map((kind) => (
              <div key={kind} className="flex items-start gap-2 text-[12px] text-t1">
                <RiskGlyph kind={kind} className="mt-0.5 size-3.5 shrink-0 text-human" />
                <span>
                  <strong className="font-medium">{RISK_META[kind].label}</strong>：{RISK_META[kind].why}
                </span>
              </div>
            ))}
          </div>
        )}
        {diff.truncated && (
          <div className="shrink-0 border-b border-line/70 px-3 py-1 text-[12px] text-t3">
            diff 过大，部分文件的内容已省略。可在仓库中查看完整改动。
          </div>
        )}
        <div className="shrink-0 border-b border-line/70 px-3 py-2 text-[12px]">
          {current && (
            <>
              <CopyText value={current.path}>{current.path}</CopyText>
              {current.oldPath && current.oldPath !== current.path && (
                <span className="text-t3"> ← {current.oldPath}</span>
              )}
            </>
          )}
        </div>
        <div className="min-h-0 flex-1">
          {!current ? (
            <EmptyState title="选择一个文件查看改动" />
          ) : current.binary ? (
            <EmptyState title="二进制文件" hint="二进制文件不显示逐行差异。" />
          ) : current.patch == null ? (
            <EmptyState title="这个文件的内容已省略" hint="diff 过大，未包含此文件的逐行内容。" />
          ) : (
            <Suspense fallback={<div className="p-4"><Skeleton height={280} /></div>}>
              <MonacoDiff
                key={current.path}
                path={current.path}
                patch={current.patch}
                jumpLine={current.path === jumpFile ? jumpLine : null}
              />
            </Suspense>
          )}
        </div>
      </div>
    </div>
  );
}

function RiskGlyph({ kind, className }: { kind: RiskKind; className?: string }) {
  const Icon = RISK_ICON[kind];
  return <Icon className={className} aria-hidden />;
}

function FilterTab({
  id,
  active,
  onClick,
  label,
  disabled,
  tone,
}: {
  id: FileFilter;
  active: FileFilter;
  onClick: (id: FileFilter) => void;
  label: string;
  disabled?: boolean;
  tone?: "attention";
}) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active === id}
      disabled={disabled}
      onClick={() => onClick(id)}
      className={cn(
        "rounded-full border px-2 py-0.5 text-[12px] transition-colors disabled:opacity-40",
        active === id ? "border-line-strong bg-raised text-t1" : "border-line text-t3 hover:text-t1",
        tone === "attention" && active !== id && "border-human/50 text-human"
      )}
    >
      {label}
    </button>
  );
}

function FileRow({
  file,
  risks,
  active,
  onClick,
}: {
  file: FileDiff;
  risks: RiskKind[];
  active: boolean;
  onClick: () => void;
}) {
  const slash = file.path.lastIndexOf("/");
  const dir = slash >= 0 ? file.path.slice(0, slash + 1) : "";
  const name = slash >= 0 ? file.path.slice(slash + 1) : file.path;
  const title = risks.length > 0 ? `${file.path}\n${risks.map((k) => RISK_META[k].label).join("、")}` : file.path;

  return (
    <li>
      <button
        type="button"
        onClick={onClick}
        title={title}
        className={cn(
          "flex w-full items-center justify-between gap-2 rounded-md px-2 py-[5px] text-left text-[12px] transition-colors hover:bg-raised",
          active && "bg-raised ring-1 ring-line"
        )}
      >
        <span className={cn("flex min-w-0 items-center gap-1 truncate font-mono", risks.length > 0 && "text-human")}>
          {risks.map((kind) => (
            <RiskGlyph key={kind} kind={kind} className="size-3 shrink-0" />
          ))}
          {dir && <span className="text-t3">{dir}</span>}
          <span>{name}</span>
        </span>
        <span className="flex shrink-0 gap-1.5 font-mono">
          {file.binary ? (
            <span className="text-t3">bin</span>
          ) : (
            <>
              <span className="text-ok">+{file.insertions}</span>
              <span className="text-bad">−{file.deletions}</span>
            </>
          )}
        </span>
      </button>
    </li>
  );
}
