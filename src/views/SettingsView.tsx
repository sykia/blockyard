import { useEffect, useState } from "react";
import { Check, RefreshCw, Download } from "lucide-react";
import { api, Settings } from "../shared";
export function SettingsView({
  settings,
  onSave,
}: {
  settings: Settings;
  onSave: (s: Settings) => void;
}) {
  const [s, setS] = useState(settings),
    [ready, setReady] = useState(false),
    [update, setUpdate] = useState<string | null>(null),
    [updating, setUpdating] = useState(false),
    [updateMessage, setUpdateMessage] = useState("");
  useEffect(() => {
    setS(settings);
  }, [settings]);
  useEffect(() => {
    api<boolean>("updater_ready")
      .then(setReady)
      .catch(() => setReady(false));
  }, []);
  const check = async () => {
    setUpdating(true);
    setUpdateMessage("");
    try {
      const version = await api<string | null>("check_update");
      setUpdate(version);
      setUpdateMessage(
        version
          ? `Version ${version} is available.`
          : "You have the latest version.",
      );
    } catch (e) {
      setUpdateMessage(String(e));
    } finally {
      setUpdating(false);
    }
  };
  const install = async () => {
    setUpdating(true);
    setUpdateMessage("Downloading signed update…");
    try {
      await api("install_update");
      setUpdateMessage("Update installed. Restart Blockyard to finish.");
      setUpdate(null);
    } catch (e) {
      setUpdateMessage(String(e));
    } finally {
      setUpdating(false);
    }
  };
  return (
    <div className="content narrow">
      <h2>Launcher preferences</h2>
      <p className="muted">
        Defaults apply to new instances. Each instance can override memory, Java
        and resolution.
      </p>
      <div className="settings-card">
        <div className="form-grid">
          <label>
            Default memory (MB)
            <input
              type="number"
              min={512}
              step={512}
              value={s.ramMb}
              onChange={(e) => setS({ ...s, ramMb: +e.target.value })}
            />
          </label>
          <label>
            Parallel downloads
            <input
              type="number"
              min={1}
              max={32}
              value={s.downloadConcurrency}
              onChange={(e) =>
                setS({ ...s, downloadConcurrency: +e.target.value })
              }
            />
          </label>
          <label>
            Default Java executable
            <input
              value={s.javaPath || ""}
              placeholder="Automatically detected"
              onChange={(e) => setS({ ...s, javaPath: e.target.value || null })}
            />
          </label>
          <label>
            Theme
            <select
              value={s.theme}
              onChange={(e) => setS({ ...s, theme: e.target.value })}
            >
              <option value="dark">Dark</option>
            </select>
          </label>
          <label>
            Default width
            <input
              type="number"
              value={s.width}
              onChange={(e) => setS({ ...s, width: +e.target.value })}
            />
          </label>
          <label>
            Default height
            <input
              type="number"
              value={s.height}
              onChange={(e) => setS({ ...s, height: +e.target.value })}
            />
          </label>
        </div>
        <label>
          Microsoft application (client) ID
          <input
            value={s.microsoftClientId}
            placeholder="Required for account sign in"
            onChange={(e) => setS({ ...s, microsoftClientId: e.target.value })}
          />
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={s.minimizeOnLaunch}
            onChange={(e) => setS({ ...s, minimizeOnLaunch: e.target.checked })}
          />{" "}
          Minimize launcher while Minecraft runs
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={s.showSnapshots}
            onChange={(e) => setS({ ...s, showSnapshots: e.target.checked })}
          />{" "}
          Show snapshots in version list
        </label>
        <div className="settings-footer">
          <button className="primary" onClick={() => onSave(s)}>
            <Check size={16} /> Save settings
          </button>
        </div>
      </div>
      <div className="settings-card">
        <div className="section-head">
          <div>
            <h3>Launcher updates</h3>
            <p>
              {ready
                ? "Updates are checked against the publisher’s signed release feed."
                : "Updates become available when the publisher configures a signing key and release feed."}
            </p>
          </div>
          {ready && (
            <button className="secondary" disabled={updating} onClick={check}>
              <RefreshCw size={16} /> Check
            </button>
          )}
        </div>
        {updateMessage && <p className="muted">{updateMessage}</p>}
        {update && (
          <button className="primary" disabled={updating} onClick={install}>
            <Download size={16} /> Install {update}
          </button>
        )}
      </div>
    </div>
  );
}
