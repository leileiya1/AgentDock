import { open } from "@tauri-apps/plugin-dialog";
import { toast } from "@/stores/toastStore";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

interface Props {
  value: string;
  onChange: (path: string) => void;
  onDetect?: () => void;
  detecting?: boolean;
  result?: string | null;
  resultTone?: "ok" | "bad";
  directory?: boolean;
  placeholder?: string;
}

/** Path input with 选择…/检测 and inline detection result (02 §6). */
export function PathField({ value, onChange, onDetect, detecting, result, resultTone, directory, placeholder }: Props) {
  const pick = async () => {
    try {
      const picked = await open({ directory: !!directory, multiple: false });
      if (typeof picked === "string") onChange(picked);
    } catch (e) {
      toast.error("打开选择对话框失败");
      console.warn(e);
    }
  };

  return (
    <div className="mt-1 flex flex-col gap-1">
      <div className="flex gap-2">
        <Input className="font-mono" value={value} onChange={(e) => onChange(e.target.value)} placeholder={placeholder ?? "/path/to/executable"} spellCheck={false} />
        <Button variant="outline" size="sm" onClick={pick}>选择…</Button>
        {onDetect && (
          <Button variant="outline" size="sm" onClick={onDetect} disabled={detecting}>
            {detecting ? "检测中…" : "检测"}
          </Button>
        )}
      </div>
      {result && <div className={cn("text-[12px]", resultTone === "bad" ? "text-bad" : "text-ok")}>{result}</div>}
    </div>
  );
}
