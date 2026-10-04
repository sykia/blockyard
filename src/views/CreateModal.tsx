import { useEffect, useState } from "react";
import { X } from "lucide-react";
import { api, Loader, Version } from "../shared";
export function CreateModal({
  versions,
  onClose,
  onCreate,
}: {
  versions: Version[];
  onClose: () => void;
  onCreate: (n: string, v: string, l: Loader) => void;
}) {
  const [name, setName] = useState(""),
    [version, setVersion] = useState(versions[0]?.id || ""),
    [kind, setKind] = useState<"vanilla" | "fabric" | "neoforge">("vanilla"),
    [loaders, setLoaders] = useState<string[]>([]),
    [loader, setLoader] = useState("");
  useEffect(() => {
    if (kind !== "vanilla" && version)
      api<string[]>(
        kind === "fabric" ? "fabric_versions" : "neoforge_versions",
        { minecraft: version },
      )
        .then((v) => {
          setLoaders(v);
          setLoader(v[0] || "");
        })
        .catch(() => setLoaders([]));
  }, [kind, version]);
  return (
    <div className="overlay" onMouseDown={onClose}>
      <div className="modal" onMouseDown={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <div>
            <div className="eyebrow">NEW SPACE</div>
            <h2>Create instance</h2>
          </div>
          <button className="iconbutton" onClick={onClose}>
            <X size={20} />
          </button>
        </div>
        <label>
          Name
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="My survival world"
            autoFocus
          />
        </label>
        <label>
          Minecraft version
          <select value={version} onChange={(e) => setVersion(e.target.value)}>
            {versions.map((v) => (
              <option key={v.id} value={v.id}>
                {v.id} {v.kind === "snapshot" ? "· Snapshot" : ""}
              </option>
            ))}
          </select>
        </label>
        <label>
          Mod loader
          <div className="segments">
            {(["vanilla", "fabric", "neoforge"] as const).map((k) => (
              <button
                className={kind === k ? "on" : ""}
                onClick={() => setKind(k)}
                key={k}
              >
                {k === "neoforge"
                  ? "NeoForge"
                  : k[0].toUpperCase() + k.slice(1)}
              </button>
            ))}
          </div>
        </label>
        {kind !== "vanilla" && (
          <label>
            {kind === "fabric" ? "Fabric Loader" : "NeoForge"} version
            <select value={loader} onChange={(e) => setLoader(e.target.value)}>
              {loaders.map((v) => (
                <option key={v}>{v}</option>
              ))}
            </select>
          </label>
        )}
        <div className="modal-actions">
          <button className="secondary" onClick={onClose}>
            Cancel
          </button>
          <button
            className="primary"
            disabled={
              !name.trim() || !version || (kind !== "vanilla" && !loader)
            }
            onClick={() =>
              onCreate(
                name,
                version,
                kind === "vanilla" ? { kind } : { kind, version: loader },
              )
            }
          >
            Create instance
          </button>
        </div>
      </div>
    </div>
  );
}
