import { AlertTriangle, CheckCircle2, CircleHelp, ShieldCheck } from "lucide-react";
import type {
  QualityEvaluation,
  QualityReplayAttempt,
  ReproducibilityManifest,
} from "@/generated/bindings";
import { CopyText } from "@/components/CopyText";
import { DRIFT_INFO, REPLAY_STATUS, REPRO_LEVEL, scoreDeltaLabel } from "@/lib/governance/reproducibility";

interface Props {
  manifest: ReproducibilityManifest | null;
  originalQuality: QualityEvaluation | null;
  latestReplay: QualityReplayAttempt | null;
}

export function ReproducibilityPanel({ manifest, originalQuality, latestReplay }: Props) {
  if (!manifest) {
    return (
      <section className="rounded-section border border-line bg-panel/60 p-4">
        <h2 className="font-semibold">可复现运行与复验对比</h2>
        <p className="mt-3 text-body text-t3">这个 revision 没有保存可复现运行清单，因此复验已禁用。新任务会在首轮验证后自动生成。</p>
      </section>
    );
  }

  const level = manifest.reproducibilityLevel ?? "fixed_commit";
  const levelInfo = REPRO_LEVEL[level];
  const limitations = manifest.limitations ?? [];
  return (
    <section className="rounded-section border border-line bg-panel/60 p-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h2 className="font-semibold">可复现运行与复验对比</h2>
          <p className="mt-1 max-w-2xl text-meta leading-relaxed text-t3">{levelInfo.detail}</p>
        </div>
        <span className="inline-flex items-center gap-1 rounded-pill border border-line bg-app/50 px-2 py-1 text-meta font-medium">
          <ShieldCheck className="size-3.5 text-selection" /> {levelInfo.label}
        </span>
      </div>

      {limitations.length > 0 && (
        <div className="mt-3 rounded-section border border-warn/35 bg-warn/5 p-3">
          <div className="flex items-center gap-1.5 text-meta font-medium text-t1"><CircleHelp className="size-4 text-warn" /> 当前复现限制</div>
          <ul className="mt-1.5 space-y-1 text-meta leading-relaxed text-t2">
            {limitations.map((item) => <li key={item}>· {item}</li>)}
          </ul>
        </div>
      )}

      <div className="mt-4 grid grid-cols-2 gap-3">
        <QualityCard title="原验证" quality={originalQuality} />
        <QualityCard title="最近复验" quality={latestReplay?.replayQuality ?? null} status={latestReplay ? REPLAY_STATUS[latestReplay.status] : "尚未复验"} />
      </div>

      {latestReplay && (
        <div className="mt-3 rounded-section border border-line bg-app/35 p-3 text-meta">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="flex items-center gap-1.5 font-medium text-t1">
              {latestReplay.status === "succeeded" ? <CheckCircle2 className="size-4 text-ok" /> : <AlertTriangle className="size-4 text-status-danger" />}
              {REPLAY_STATUS[latestReplay.status]}
            </div>
            <span className="text-t3">{scoreDeltaLabel(latestReplay.scoreDelta)}</span>
          </div>
          <div className="mt-1 text-meta text-t3">
            环境摘要：{latestReplay.environmentMatch ? "匹配" : "不匹配"} · {new Date(latestReplay.finishedAt).toLocaleString()}
          </div>
          {latestReplay.errorDetail && <p className="mt-2 rounded bg-status-human-bg p-2 text-meta text-status-danger">{latestReplay.errorDetail}</p>}
          {latestReplay.drift.length > 0 && (
            <div className="mt-3 space-y-2">
              {latestReplay.drift.map((item) => {
                const info = DRIFT_INFO[item.kind];
                return (
                  <div key={item.kind} className="rounded-control border border-line bg-panel/60 p-2">
                    <div className="font-medium text-t1">{info.label}</div>
                    <div className="mt-0.5 text-meta text-t3">变化项：{item.changedKeys.join("、") || "未提供"}</div>
                    <div className="mt-1 text-meta text-t2">恢复建议：{info.recovery}</div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      )}

      <details className="mt-3 text-meta text-t3">
        <summary className="cursor-pointer select-none">运行清单与校验摘要</summary>
        <div className="mt-2 grid grid-cols-2 gap-x-6 gap-y-2">
          <ManifestRow label="commit" value={manifest.commitSha} />
          <ManifestRow label="manifest" value={manifest.manifestSha256} />
          <ManifestRow label="输入" value={manifest.inputSha256} />
          <ManifestRow label="代码差异" value={manifest.patchSha256} />
          <ManifestRow label="验证配置" value={manifest.validationConfigSha256} />
          <div className="flex gap-2"><span className="w-16 text-t3">验证环境</span><span className="text-t2">{manifest.environment["validation_location"] === "remote" ? "远程" : "本机"} · {manifest.environment["validation_platform"] ?? "未知"}</span></div>
        </div>
      </details>
    </section>
  );
}

function QualityCard({ title, quality, status }: { title: string; quality: QualityEvaluation | null; status?: string }) {
  return (
    <div className="rounded-section border border-line bg-app/35 p-3">
      <div className="text-meta text-t3">{title}</div>
      {quality ? (
        <div className="mt-1 flex items-end justify-between gap-2"><span className="text-2xl font-semibold text-t1">{quality.score}</span><span className={quality.passed ? "text-ok" : "text-status-danger"}>{status ?? (quality.passed ? "通过" : "未通过")}</span></div>
      ) : <div className="mt-2 text-meta text-t3">{status ?? "没有记录"}</div>}
    </div>
  );
}

function ManifestRow({ label, value }: { label: string; value: string }) {
  return <div className="flex min-w-0 gap-2"><span className="w-16 shrink-0 text-t3">{label}</span><CopyText value={value} className="truncate font-mono text-t2">{value.slice(0, 16)}…</CopyText></div>;
}
