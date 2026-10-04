import { useEffect, useState } from "react";
import { FolderOpen, Copy } from "lucide-react";
import { openPath } from "@tauri-apps/plugin-opener";
import { api, Instance, Status } from "../shared";
export function LogsView({
  instance,
  status,
  lines,
  action,
}: {
  instance: Instance | undefined;
  status: Status | null;
  lines: string[];
  action: (f: () => Promise<unknown>) => void;
}) {
  const [files, setFiles] = useState<string[]>([]),
    [path, setPath] = useState(""),
    [text, setText] = useState("");
  useEffect(() => {
    if (instance)
      api<string[]>("instance_files", { id: instance.id })
        .then(setFiles)
        .catch(() => {});
  }, [instance?.id, status?.phase]);
  const read = (p: string) =>
    action(async () => {
      setPath(p);
      setText(await api<string>("read_log", { id: instance?.id, path: p }));
    });
  const content = path ? text : lines.join("");
  return (
    <div className="content">
      <div className="section-head">
        <div>
          <h2>{instance?.name || "No instance selected"}</h2>
          <p>{status?.message || "Game output and crash reports"}</p>
        </div>
        <button
          className="secondary"
          disabled={!instance}
          onClick={() =>
            action(async () => {
              const dir = await api<string>("instance_directory", {
                id: instance?.id,
              });
              await openPath(dir);
            })
          }
        >
          <FolderOpen size={16} /> Open instance
        </button>
      </div>
      {status?.phase === "crashed" && (
        <div className="crash-banner">
          <strong>Minecraft stopped unexpectedly</strong>
          <p>
            Open the latest log or crash report below to inspect the cause.
            Check Java, mods and memory settings first.
          </p>
        </div>
      )}
      <div className="log-layout">
        <div className="log-files">
          <button
            className={!path ? "selected" : ""}
            onClick={() => setPath("")}
          >
            Live session
          </button>
          {files.map((f) => (
            <button
              className={path === f ? "selected" : ""}
              onClick={() => read(f)}
              key={f}
            >
              {f.split("/").slice(-2).join("/")}
            </button>
          ))}
        </div>
        <div className="log-panel">
          <div className="log-toolbar">
            <span>{path || "Live output"}</span>
            <button
              className="textbutton"
              onClick={() => navigator.clipboard.writeText(content)}
            >
              <Copy size={15} /> Copy
            </button>
            <button
              className="textbutton"
              onClick={() =>
                navigator.clipboard.writeText(
                  `Blockyard 0.3.0\nInstance: ${instance?.name || "unknown"} (${instance?.version || "unknown"}, ${instance?.loader.kind || "unknown"})\nStatus: ${status?.phase || "unknown"} / ${status?.exitCode ?? "n/a"}\n${status?.message || ""}\n\n${content.slice(-12000)}`,
                )
              }
            >
              <Copy size={15} /> Diagnostics
            </button>
            {path && (
              <button className="textbutton" onClick={() => openPath(path)}>
                <FolderOpen size={15} /> Open
              </button>
            )}
          </div>
          <pre>{content || "No output yet."}</pre>
        </div>
      </div>
    </div>
  );
}
