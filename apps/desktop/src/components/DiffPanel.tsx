import { Suspense, lazy, useEffect, useMemo, useState } from "react";
import { AlertTriangle } from "lucide-react";
import type { DiffPayload, FileDiff } from "@/generated/bindings";
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

/** File tree (numstat, flagged amber) + read-only Monaco DiffEditor (02 §4.4). */
export function DiffPanel({ diff, jumpFile, jumpLine }: Props) {
  const files = diff.files;
  const [selected, setSelected] = useState<string | null>(files[0]?.path ?? null);

  useEffect(() => {
    if (jumpFile && files.some((f) => f.path === jumpFile)) setSelected(jumpFile);
  }, [jumpFile, files]);

  const current = useMemo(() => files.find((f) => f.path === selected) ?? null, [files, selected]);

  if (files.length === 0) {
    return <EmptyState title="这一轮没有可显示的改动" hint="可能是空改动，或全部文件都被排除规则过滤了。" />;
  }

  return (
    <div className="flex h-full min-h-0">
      <div className="flex w-72 shrink-0 flex-col border-r border-line/70">
        <div className="flex shrink-0 justify-between border-b border-line/70 px-3 py-2 text-[12px] text-t3">
          <span>{files.length} 个文件</span>
          {diff.truncated && <span className="text-human" title="diff 过大，部分文件内容已省略">已截断</span>}
        </div>
        <ul className="flex-1 list-none overflow-y-auto p-1">
          {files.map((f) => (
            <FileRow key={f.path} file={f} active={f.path === selected} onClick={() => setSelected(f.path)} />
          ))}
        </ul>
      </div>

      <div className="flex min-w-0 flex-1 flex-col">
        {current && current.flagged && (
          <div className="flex shrink-0 items-center gap-2 border-b border-human bg-human-bg px-3 py-2 text-[12px] text-human">
            <AlertTriangle className="size-3.5 shrink-0" />
            该文件属于规则/控制面文件，Agent 修改了它——请确认这是你想要的。
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
              <MonacoDiff key={current.path} path={current.path} patch={current.patch} jumpLine={current.path === jumpFile ? jumpLine : null} />
            </Suspense>
          )}
        </div>
      </div>
    </div>
  );
}

function FileRow({ file, active, onClick }: { file: FileDiff; active: boolean; onClick: () => void }) {
  const slash = file.path.lastIndexOf("/");
  const dir = slash >= 0 ? file.path.slice(0, slash + 1) : "";
  const name = slash >= 0 ? file.path.slice(slash + 1) : file.path;
  return (
    <li>
      <button
        type="button"
        onClick={onClick}
        title={file.path}
        className={cn(
          "flex w-full items-center justify-between gap-2 rounded-md px-2 py-[5px] text-left text-[12px] transition-colors hover:bg-raised",
          active && "bg-raised ring-1 ring-line"
        )}
      >
        <span className={cn("flex min-w-0 items-center gap-1 truncate font-mono", file.flagged && "text-human")}>
          {file.flagged && <AlertTriangle className="size-3 shrink-0" />}
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
