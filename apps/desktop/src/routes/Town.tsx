import { useMemo, useRef, useState, type FormEvent } from "react";
import { useNavigate } from "react-router-dom";
import { useProjectSummaries, useProjects } from "@/hooks/useProjects";
import { useProjectAppearance } from "@/hooks/useStudio";
import { townAgents } from "@/lib/studio/projectSummary";
import { useUiStore } from "@/stores/uiStore";
import { TownView, type TownViewHandle } from "@/components/studio/town/TownView";
import { ErrorState } from "@/components/ErrorState";
import { SkeletonRows } from "@/components/Skeleton";
import { PixelIcon } from "@/components/pixel/PixelIcon";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { ProjectDecorDialog } from "@/components/studio/settings/ProjectDecorDialog";
import { AgentProfilesDialog } from "@/components/studio/settings/AgentProfilesDialog";

const LEGEND = [
  "烟囱冒烟 · 有任务在跑",
  "邮箱小旗 · 有事等你",
  "头顶下雨 · 测试在失败",
  "落叶窗暗 · 3 天没动",
  "院子长草 · 一个月没动",
  "脚手架 + 施工队 · 新项目，派第一个任务就完工",
  "院子里的机器人 · 这个项目分配的 Agent",
];

/** 项目小镇：所有项目一张地图，每个项目一栋房子，四周是公园、游乐园这些公共设施。 */
export function Town() {
  const navigate = useNavigate();
  const projects = useProjects();
  const summaries = useProjectSummaries();
  const appearance = useProjectAppearance();
  const setHomeMode = useUiStore((s) => s.setHomeMode);
  const townRef = useRef<TownViewHandle>(null);
  const [query, setQuery] = useState("");
  const [miss, setMiss] = useState(false);
  const [decorating, setDecorating] = useState<string | null>(null);
  const [profilesOpen, setProfilesOpen] = useState(false);
  const decorProject = projects.data?.find((p) => p.id === decorating);

  const totals = useMemo(() => {
    const list = summaries.data ?? [];
    return {
      waiting: list.reduce((n, s) => n + s.waitingForYou, 0),
      running: list.reduce((n, s) => n + s.running, 0),
      agents: new Set(list.flatMap((s) => townAgents(s))).size,
    };
  }, [summaries.data]);

  const open = (projectId: string) => {
    setHomeMode("studio");
    navigate(`/p/${projectId}`);
  };

  const search = (e: FormEvent) => {
    e.preventDefault();
    const q = query.trim().toLocaleLowerCase();
    const hit = projects.data?.find((p) => p.name.toLocaleLowerCase().includes(q));
    setMiss(!hit && q.length > 0);
    if (hit) townRef.current?.focusProject(hit.id);
  };

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="flex-1 overflow-y-auto px-6 py-6">
        <div className="mx-auto flex max-w-6xl flex-col gap-4">
          <section className="flex flex-wrap items-center gap-x-5 gap-y-2" aria-label="小镇概况">
            <div className="min-w-0 flex-1">
              <h1 className="text-page font-extrabold tracking-tight text-t1">项目小镇</h1>
              <p className="mt-1 text-meta text-t2">
                {projects.data?.length ?? 0} 个项目 · {totals.agents} 位 Agent 在镇上 · {totals.running} 件在跑 ·{" "}
                <span className={totals.waiting > 0 ? "font-bold text-status-human" : undefined}>{totals.waiting} 件等你</span>
              </p>
            </div>
            <form onSubmit={search} className="flex items-center gap-2" role="search">
              <div className="relative">
                <PixelIcon name="search" size={12} className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-t3" />
                <Input
                  value={query}
                  onChange={(e) => {
                    setQuery(e.target.value);
                    setMiss(false);
                  }}
                  list="town-projects"
                  className="w-64 pl-9"
                  aria-label="搜索项目"
                  placeholder="搜项目名，镜头飞过去"
                />
                <datalist id="town-projects">
                  {projects.data?.map((p) => <option key={p.id} value={p.name} />)}
                </datalist>
              </div>
              <Button type="submit" variant="outline">去看看</Button>
            </form>
            <Button variant="outline" onClick={() => setProfilesOpen(true)}>
              <PixelIcon name="squad" size={12} />
              Agent 档案
            </Button>
          </section>
          {miss && <p className="text-meta text-status-danger">没有找到「{query}」这个项目</p>}

          {projects.isLoading || summaries.isLoading ? (
            <SkeletonRows rows={6} />
          ) : projects.isError ? (
            <ErrorState error={projects.error} onRetry={() => projects.refetch()} />
          ) : (
            <TownView ref={townRef} projects={projects.data ?? []} summaries={summaries.data ?? []} appearance={appearance.data ?? []} onOpen={open} onDecorate={setDecorating} />
          )}

          <ul className="flex flex-wrap gap-2" aria-label="外观图例">
            {LEGEND.map((l) => (
              <li key={l} className="rounded-pill bg-raised px-2.5 py-1 text-micro font-bold text-t2 ring-1 ring-line">
                {l}
              </li>
            ))}
          </ul>
        </div>
      </div>
      {decorProject && (
        <ProjectDecorDialog
          open
          onClose={() => setDecorating(null)}
          projectId={decorProject.id}
          projectName={decorProject.name}
          summary={summaries.data?.find((s) => s.projectId === decorProject.id)}
        />
      )}
      <AgentProfilesDialog open={profilesOpen} onClose={() => setProfilesOpen(false)} />
    </div>
  );
}
