import { EDGES, NODES, nodeById, spotById, type Pt } from "./layout";

const ADJ: Map<string, Array<{ to: string; cost: number }>> = (() => {
  const adj = new Map<string, Array<{ to: string; cost: number }>>();
  for (const node of NODES) adj.set(node.id, []);
  for (const [a, b] of EDGES) {
    const pa = nodeById(a)!.at, pb = nodeById(b)!.at;
    const cost = Math.hypot(pa.x - pb.x, pa.y - pb.y);
    adj.get(a)!.push({ to: b, cost });
    adj.get(b)!.push({ to: a, cost });
  }
  return adj;
})();

/** 走道节点之间的最短路（Dijkstra，节点很少，直接线性找最小）。不可达返回 null。 */
export function nodePath(from: string, to: string): string[] | null {
  if (!ADJ.has(from) || !ADJ.has(to)) return null;
  const dist = new Map<string, number>([[from, 0]]);
  const prev = new Map<string, string>();
  const done = new Set<string>();
  while (true) {
    let cur: string | null = null;
    let best = Infinity;
    for (const [id, d] of dist) {
      if (!done.has(id) && d < best) {
        best = d;
        cur = id;
      }
    }
    if (cur === null) return null;
    if (cur === to) break;
    done.add(cur);
    for (const { to: next, cost } of ADJ.get(cur)!) {
      const nd = best + cost;
      if (nd < (dist.get(next) ?? Infinity)) {
        dist.set(next, nd);
        prev.set(next, cur);
      }
    }
  }
  const path = [to];
  while (path[0] !== from) path.unshift(prev.get(path[0])!);
  return path;
}

/**
 * 从一个点位走到另一个点位的折线：起点 → 起点的入口节点 → 走道 → 终点的入口节点 → 终点。
 * 同一个点位返回只含起点的折线。
 */
export function route(fromSpot: string, toSpot: string): Pt[] | null {
  const a = spotById(fromSpot), b = spotById(toSpot);
  if (!a || !b) return null;
  if (a.id === b.id) return [a.at];
  const nodes = nodePath(a.node, b.node);
  if (!nodes) return null;
  const pts: Pt[] = [a.at, ...nodes.map((id) => nodeById(id)!.at), b.at];
  return pts.filter((p, i) => i === 0 || p.x !== pts[i - 1].x || p.y !== pts[i - 1].y);
}

export function pathLength(points: Pt[]): number {
  let total = 0;
  for (let i = 1; i < points.length; i++) total += Math.hypot(points[i].x - points[i - 1].x, points[i].y - points[i - 1].y);
  return total;
}
