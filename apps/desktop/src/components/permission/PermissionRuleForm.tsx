import { ScrollText } from "lucide-react";
import type { PermissionRequest } from "@/generated/bindings";
import { rulePreview } from "@/lib/permission/rules";

/**
 * 「保存项目规则」二次确认 (06 §9 第 13 条)。绝不显示一句「以后允许」——这里逐行展示
 * 后端将写入的精确能力：可执行文件、参数模板、允许目录、精确域名端口、环境变量名。
 * 用户先看到真实规则内容，再确认保存。
 */
export function PermissionRuleForm({ request }: { request: PermissionRequest }) {
  const lines = rulePreview(request);
  return (
    <div className="rounded-md border border-line bg-app p-3">
      <div className="mb-2 flex items-center gap-1.5 text-[12px] font-semibold text-t2">
        <ScrollText className="size-3.5" aria-hidden />
        将为当前项目保存这条精确规则
      </div>
      <dl className="flex flex-col gap-1.5">
        {lines.map((line, i) => (
          <div key={`${line.label}-${i}`} className="flex flex-wrap items-baseline gap-x-2 gap-y-0.5 text-[12px]">
            <dt className="shrink-0 text-t3">{line.label}</dt>
            <dd className={line.mono ? "min-w-0 break-all font-mono text-t1" : "min-w-0 break-words text-t1"}>
              {line.value}
            </dd>
          </div>
        ))}
      </dl>
      <p className="mt-2 text-[11px] text-t3">
        规则只匹配完全相同的命令与域名，不会放行其它操作；之后可在项目设置里查看命中、停用或撤销。
      </p>
    </div>
  );
}
