import { useEffect, useState } from "react";
import { Puzzle, Plus, Trash2, FolderOpen } from "lucide-react";
import { open as fileDialog } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import { api, Instance, Mod } from "../shared";
export function ModsView({
  instance,
  action,
  onBack,
}: {
  instance: Instance;
  action: (f: () => Promise<unknown>) => void;
  onBack: () => void;
}) {
  const [mods, setMods] = useState<Mod[]>([]);
  const load = () => api<Mod[]>("mods", { id: instance.id }).then(setMods);
  useEffect(() => {
    load().catch(() => {});
  }, [instance.id]);
  const doAction = (f: () => Promise<unknown>) =>
    action(async () => {
      await f();
      await load();
    });
  return (
    <div className="content narrow">
      <button className="textbutton" onClick={onBack}>
        ← Back to instance
      </button>
      <div className="section-head">
        <div>
          <h2>{instance.name} mods</h2>
          <p>{mods.length} files in this instance</p>
        </div>
        <button
          className="primary"
          onClick={() =>
            doAction(async () => {
              const selected = await fileDialog({
                multiple: false,
                filters: [{ name: "Java mods", extensions: ["jar"] }],
              });
              if (selected)
                await api("add_mod", { id: instance.id, source: selected });
            })
          }
        >
          <Plus size={16} /> Add mod
        </button>
      </div>
      <div className="list-card">
        {mods.map((m) => (
          <div className="mod-row" key={m.name}>
            <Puzzle size={19} />
            <div>
              <strong>{m.name.replace(".disabled", "")}</strong>
              <span>
                {Math.round(m.size / 1024)} KB ·{" "}
                {m.enabled ? "Enabled" : "Disabled"}
              </span>
            </div>
            <button
              className="secondary compact"
              onClick={() =>
                doAction(() =>
                  api("toggle_mod", { id: instance.id, name: m.name }),
                )
              }
            >
              {m.enabled ? "Disable" : "Enable"}
            </button>
            <button
              className="iconbutton"
              onClick={() =>
                doAction(() =>
                  api("remove_mod", { id: instance.id, name: m.name }),
                )
              }
            >
              <Trash2 size={16} />
            </button>
          </div>
        ))}
        {!mods.length && (
          <div className="empty">
            No mods installed. Add a compatible .jar to get started.
          </div>
        )}
      </div>
      <button
        className="textbutton"
        onClick={() =>
          doAction(async () =>
            openPath(
              (await api<string>("instance_directory", { id: instance.id })) +
                "/mods",
            ),
          )
        }
      >
        <FolderOpen size={16} /> Open mods folder
      </button>
    </div>
  );
}
