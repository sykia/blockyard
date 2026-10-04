import { useEffect, useState } from "react";
import {
  Box,
  Play,
  Puzzle,
  ScrollText,
  FolderOpen,
  Trash2,
  Check,
} from "lucide-react";
import { Instance, InstanceGlyph } from "../shared";
export function InstanceDetail({
  instance,
  busy,
  onSave,
  onDelete,
  onPlay,
  onMods,
  onLogs,
  onOpen,
}: {
  instance: Instance;
  busy: boolean;
  onSave: (i: Instance) => void;
  onDelete: () => void;
  onPlay: () => void;
  onMods: () => void;
  onLogs: () => void;
  onOpen: () => void;
}) {
  const [edit, setEdit] = useState(instance);
  useEffect(() => setEdit(instance), [instance]);
  return (
    <>
      <div className="detail-top">
        <div className="instance-icon large">
          <InstanceGlyph icon={instance.icon} />
        </div>
        <div>
          <div className="eyebrow">INSTANCE</div>
          <h2>{instance.name}</h2>
          <p>
            {instance.version} · {instance.loader.kind}
          </p>
        </div>
        <button className="primary" disabled={busy} onClick={onPlay}>
          <Play size={16} /> Play
        </button>
      </div>
      <div className="quick-actions">
        <button onClick={onMods}>
          <Puzzle size={17} /> Mods
        </button>
        <button onClick={onLogs}>
          <ScrollText size={17} /> Logs
        </button>
        <button onClick={onOpen}>
          <FolderOpen size={17} /> Open folder
        </button>
      </div>
      <div className="form-grid">
        <label>
          Name
          <input
            value={edit.name}
            onChange={(e) => setEdit({ ...edit, name: e.target.value })}
          />
        </label>
        <label>
          Icon
          <select
            value={edit.icon}
            onChange={(e) => setEdit({ ...edit, icon: e.target.value })}
          >
            {["cube", "sword", "pickaxe", "leaf", "flame"].map((icon) => (
              <option key={icon} value={icon}>
                {icon[0].toUpperCase() + icon.slice(1)}
              </option>
            ))}
          </select>
        </label>
        <label>
          Memory (MB)
          <input
            type="number"
            min={512}
            step={512}
            value={edit.ramMb}
            onChange={(e) => setEdit({ ...edit, ramMb: +e.target.value })}
          />
        </label>
        <label>
          Java executable
          <input
            value={edit.javaPath || ""}
            placeholder="Automatically detected"
            onChange={(e) =>
              setEdit({ ...edit, javaPath: e.target.value || null })
            }
          />
        </label>
        <label>
          JVM arguments
          <input
            value={edit.jvmArgs}
            placeholder="Optional"
            onChange={(e) => setEdit({ ...edit, jvmArgs: e.target.value })}
          />
        </label>
        <label>
          Width
          <input
            type="number"
            value={edit.width}
            onChange={(e) => setEdit({ ...edit, width: +e.target.value })}
          />
        </label>
        <label>
          Height
          <input
            type="number"
            value={edit.height}
            onChange={(e) => setEdit({ ...edit, height: +e.target.value })}
          />
        </label>
      </div>
      <div className="detail-footer">
        <button
          className="danger"
          onClick={() => {
            if (confirm(`Delete ${instance.name} and all its game files?`))
              onDelete();
          }}
        >
          <Trash2 size={16} /> Delete
        </button>
        <button
          className="primary"
          disabled={busy}
          onClick={() => onSave(edit)}
        >
          <Check size={16} /> Save changes
        </button>
      </div>
    </>
  );
}
