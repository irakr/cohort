import { useState } from "react";
import { ChevronRight, FileText, Folder } from "lucide-react";
import type { FileNode } from "../api/types";

export function FileTree({
  nodes,
  onOpenFile,
}: {
  nodes: FileNode[];
  onOpenFile: (path: string) => void;
}) {
  const [expanded, setExpanded] = useState<Record<string, boolean>>(() => {
    // Directories start expanded, like the prototype.
    const map: Record<string, boolean> = {};
    const walk = (list: FileNode[]) => {
      for (const n of list) {
        if (n.children.length > 0) {
          map[n.path] = true;
          walk(n.children);
        }
      }
    };
    walk(nodes);
    return map;
  });

  const rows: { node: FileNode; depth: number; isDir: boolean; open: boolean }[] = [];
  const walk = (list: FileNode[], depth: number) => {
    for (const n of list) {
      const isDir = n.children.length > 0;
      const open = !!expanded[n.path];
      rows.push({ node: n, depth, isDir, open });
      if (isDir && open) {
        walk(n.children, depth + 1);
      }
    }
  };
  walk(nodes, 0);

  return (
    <div style={{ display: "flex", flexDirection: "column" }}>
      {rows.map(({ node, depth, isDir, open }) => (
        <button
          key={node.path}
          onClick={() =>
            isDir
              ? setExpanded((e) => ({ ...e, [node.path]: !open }))
              : onOpenFile(node.path)
          }
          style={{
            display: "flex",
            alignItems: "center",
            gap: 6,
            border: "none",
            background: "transparent",
            cursor: "pointer",
            font: "inherit",
            fontSize: 12.5,
            color: "var(--color-text)",
            padding: "3px 6px",
            paddingLeft: 4 + depth * 14,
            borderRadius: 6,
            textAlign: "left",
          }}
        >
          <span style={{ width: 10, display: "inline-flex" }}>
            {isDir && (
              <ChevronRight
                size={10}
                strokeWidth={2.5}
                style={{ transform: open ? "rotate(90deg)" : "none", transition: "transform .15s ease" }}
              />
            )}
          </span>
          {isDir ? (
            <Folder size={14} color="#64a8e8" fill="#64a8e8" />
          ) : (
            <FileText size={13} color="var(--color-neutral-500)" />
          )}
          {node.name}
        </button>
      ))}
    </div>
  );
}
