import { useState, useEffect, useRef } from "react";
import type { ExampleGroup } from "../generated/sourceData";

interface TerminalProps {
  groups: ExampleGroup[];
  className?: string;
}

export default function Terminal({ groups, className = "" }: TerminalProps) {
  const [active, setActive] = useState(0);
  const bodyRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (bodyRef.current) bodyRef.current.scrollTop = 0;
  }, [active]);

  const safeIndex = Math.min(active, groups.length - 1);
  const current = groups[safeIndex];

  return (
    <div
      className={`overflow-hidden rounded-xl border border-border bg-surface-alt shadow-2xl ${className}`}
    >
      <div className="flex items-center gap-3 border-b border-border px-4 py-3">
        <div className="flex items-center gap-2">
          <div className="h-3 w-3 rounded-full bg-[#ff5f57]" />
          <div className="h-3 w-3 rounded-full bg-[#febc2e]" />
          <div className="h-3 w-3 rounded-full bg-[#28c840]" />
        </div>
        <div className="flex flex-1 gap-1 overflow-x-auto">
          {groups.map((group, i) => (
            <button
              key={`${i}-${group.title}`}
              onClick={() => setActive(i)}
              className={`whitespace-nowrap rounded-md px-3 py-1 text-xs font-medium transition-colors ${
                i === safeIndex
                  ? "bg-surface text-accent"
                  : "text-text-dim hover:text-text-secondary"
              }`}
            >
              {tabLabel(group, i)}
            </button>
          ))}
        </div>
      </div>

      <div
        ref={bodyRef}
        className="h-[360px] overflow-y-auto p-5 text-left font-mono text-sm leading-relaxed"
      >
        {current?.steps.map((step, i) => (
          <div key={i} className={i > 0 ? "mt-4" : ""}>
            <div className="flex">
              <span className="mr-2 select-none text-accent">$</span>
              <span className="whitespace-pre-wrap text-text-primary">
                {step.command}
              </span>
            </div>
            {step.output.map((line, j) => (
              <div
                key={j}
                className="whitespace-pre-wrap pl-4 text-text-dim"
              >
                {line || " "}
              </div>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}

function tabLabel(group: ExampleGroup, idx: number): string {
  const raw = group.comment || `Example ${idx + 1}`;
  const trimmed = raw.split(/[—.:]/)[0].trim();
  return trimmed.length > 40 ? `${trimmed.slice(0, 37)}…` : trimmed;
}
