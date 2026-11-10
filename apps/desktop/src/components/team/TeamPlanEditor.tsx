import { useState } from "react";
import { commands, type TeamPlan, type TeamPlanPreview, type TeamView } from "@/generated/bindings";
import { unwrap } from "@/lib/commands";
import { useTeamAction } from "@/hooks/useTeam";
import { errorLine } from "@/copy/errors";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/input";

export function TeamPlanImpact({ preview }: { preview: TeamPlanPreview }) {
  const groups = [["新增", preview.added], ["移除", preview.removed], ["规范变化", preview.changed], ["受影响，将失效重做", preview.affected], ["规范和输入未变，保留", preview.preserved]] as const;
  return <div className="mt-3 space-y-3" data-testid="team-plan-impact"><p className="break-all text-meta text-t3">依据计划 v{preview.baseRevision} · 提案 {preview.proposalHash}</p>
    <table className="w-full text-left text-body"><tbody>{groups.map(([label, ids]) => <tr key={label} className="border-b border-line"><th className="py-2 pr-3 font-medium">{label}</th><td className="py-2">{ids.join("、") || "无"}</td></tr>)}</tbody></table>
    <p className="text-meta text-t2">激活后重新形成候选，并重做最终覆盖核对、实测和独立审查。历史证据保留，累计额度不重置。</p>
    {preview.blockingAttemptIds.length > 0 && <p className="break-all text-meta">必须先停止或对账的尝试：{preview.blockingAttemptIds.join("、")}</p>}
    {preview.blockers.length > 0 && <ul className="space-y-1 text-body" role="alert">{preview.blockers.map((b) => <li key={b}>{b}</li>)}</ul>}
  </div>;
}
export function TeamPlanEditor({ view, editable }: { view: TeamView; editable: boolean }) {
  const [text, setText] = useState("");
  const [preview, setPreview] = useState<{ result: TeamPlanPreview; text: string; plan: TeamPlan } | null>(null);
  const [inspecting, setInspecting] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const action = useTeamAction(view.taskId);
  const current = preview?.text === text && preview.result.baseRevision === view.planRevision;
  const inspect = async () => {
    setProblem(null); setInspecting(true); const source = text;
    try {
      const plan = JSON.parse(source) as TeamPlan;
      const result = await unwrap(commands.teamPlanPreview({ taskId: view.taskId, expectedRevision: view.planRevision, plan }));
      setPreview({ result, text: source, plan });
    } catch (e) { setPreview(null); setProblem(e instanceof SyntaxError ? "计划 JSON 无法解析" : errorLine(e)); }
    finally { setInspecting(false); }
  };
  return <details className="rounded-section border border-line p-4"><summary>执行计划与有界局部修订</summary><pre className="mt-3 overflow-x-auto whitespace-pre-wrap text-meta">{JSON.stringify(view.plan, null, 2)}</pre>
    {editable && <><p className="my-3 text-body">预览由后端计算依赖和实际产物影响；无关成果可继续保留。已封存验收不能在此删除或放宽。</p><Button variant="ghost" size="sm" onClick={() => { setText(JSON.stringify(view.plan, null, 2)); setPreview(null); }}>复制当前计划到编辑区</Button>
      <Textarea aria-label="局部修订计划 JSON" rows={10} value={text} onChange={(e) => setText(e.target.value)} placeholder="完整 TeamPlan JSON" />
      <div className="mt-3 flex flex-wrap gap-2"><Button variant="outline" disabled={inspecting || action.isPending || !text.trim()} onClick={() => void inspect()}>预览变更与影响</Button><Button variant="primary" disabled={!current || !preview || preview.result.blockers.length > 0 || inspecting || action.isPending} onClick={async () => { if (!current || !preview) return; setProblem(null); try { await action.mutateAsync({ action: "revise_plan", expected_revision: view.planRevision, plan: preview.plan }); setPreview(null); } catch (e) { setProblem(errorLine(e)); } }}>激活已预览的修订</Button></div>
      {preview && current && <TeamPlanImpact preview={preview.result} />}{preview && !current && <p className="mt-3 text-meta">内容或后端计划版本已变化，请重新预览。</p>}{problem && <p role="alert" className="mt-3 text-body">{problem}</p>}
    </>}
  </details>;
}
