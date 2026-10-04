import { useEffect, useState } from "react";
import { Check, RefreshCw, Download } from "lucide-react";
import { api, Settings } from "../shared";
import { listen } from "@tauri-apps/api/event";
import { applyTheme } from "../theme";
import type { Theme } from "../theme";
import { applyMotion } from "../motion";
export function SettingsView({
  settings,
  onSave,
}: {
  settings: Settings;
  onSave: (s: Settings) => void;
}) {
  const [s, setS] = useState(settings),
    [ready, setReady] = useState(false),
    [curseKey, setCurseKey] = useState(""),
    [keySaved, setKeySaved] = useState(false),
    [keyMessage, setKeyMessage] = useState(""),
    [update, setUpdate] = useState<string | null>(null),
    [updating, setUpdating] = useState(false),
    [updateMessage, setUpdateMessage] = useState(""),
    [downloaded, setDownloaded] = useState(0),
    [downloadTotal, setDownloadTotal] = useState<number | null>(null),
    [updatePhase, setUpdatePhase] = useState("");
  useEffect(() => {
    setS(settings);
  }, [settings]);
  useEffect(() => applyTheme(s.theme), [s.theme]);
  useEffect(() => () => applyTheme(settings.theme), [settings.theme]);
  useEffect(() => applyMotion(s.animationsEnabled), [s.animationsEnabled]);
  useEffect(
    () => () => applyMotion(settings.animationsEnabled),
    [settings.animationsEnabled],
  );
  useEffect(() => {
    api<boolean>("updater_ready")
      .then((available) => {
        setReady(available);
        if (available) void check();
      })
      .catch(() => setReady(false));
  }, []);
  useEffect(() => {
    const listener = listen<{
      phase: string;
      received: number;
      total: number | null;
      message: string;
    }>("update-progress", (event) => {
      setUpdatePhase(event.payload.phase);
      setDownloaded(event.payload.received);
      setDownloadTotal(event.payload.total);
      setUpdateMessage(event.payload.message);
    });
    return () => {
      void listener.then((unlisten) => unlisten());
    };
  }, []);
  useEffect(() => {
    api<boolean>("catalog_key_status")
      .then(setKeySaved)
      .catch(() => {});
  }, []);
  const saveKey = async () => {
    try {
      await api("catalog_set_key", { key: curseKey });
      setKeySaved(Boolean(curseKey.trim()));
      setCurseKey("");
      setKeyMessage(
        curseKey.trim()
          ? "API key saved in system keyring."
          : "API key removed.",
      );
    } catch (e) {
      setKeyMessage(String(e));
    }
  };
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
    setUpdatePhase("download");
    setDownloaded(0);
    setDownloadTotal(null);
    setUpdateMessage("Downloading signed update…");
    try {
      await api("install_update");
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
              onChange={(e) => setS({ ...s, theme: e.target.value as Theme })}
            >
              <option value="dark">Dark</option>
              <option value="light">Light</option>
              <option value="system">System</option>
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
            checked={s.animationsEnabled}
            onChange={(e) =>
              setS({ ...s, animationsEnabled: e.target.checked })
            }
          />{" "}
          Enable interface animations
        </label>
        <p className="muted motion-note">
          Animates navigation, cards, controls and progress. The system Reduce
          Motion setting always takes priority.
        </p>
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
        <h3>CurseForge catalog</h3>
        <p className="muted">
          {keySaved
            ? "API key configured"
            : "Add your CurseForge API key to browse and install files."}{" "}
          The key is stored in your system keyring.
        </p>
        <label>
          CurseForge API key
          <input
            type="password"
            value={curseKey}
            onChange={(e) => setCurseKey(e.target.value)}
            placeholder="Paste your key"
          />
        </label>
        <div className="settings-footer">
          <button className="primary" onClick={saveKey}>
            Save key
          </button>
          <button
            className="secondary"
            onClick={() => {
              setCurseKey("");
              api("catalog_set_key", { key: "" })
                .then(() => {
                  setKeySaved(false);
                  setKeyMessage("API key removed.");
                })
                .catch((e) => setKeyMessage(String(e)));
            }}
          >
            Remove key
          </button>
        </div>
        {keyMessage && <p className="muted">{keyMessage}</p>}
      </div>
      <div className="settings-card">
        <div className="section-head">
          <div>
            <h3>Launcher updates</h3>
            <p>
              {ready
                ? "Signed updates are checked automatically when Settings opens."
                : "Install a packaged release to enable updates."}
            </p>
          </div>
          {ready && (
            <button className="secondary" disabled={updating} onClick={check}>
              <RefreshCw size={16} /> Check
            </button>
          )}
        </div>
        {updateMessage && <p className="muted">{updateMessage}</p>}
        {updatePhase === "download" && downloadTotal && (
          <div className="track">
            <div
              style={{
                width: `${Math.min(100, Math.round((downloaded / downloadTotal) * 100))}%`,
              }}
            />
          </div>
        )}
        {updatePhase === "download" && (
          <p className="muted">
            {downloadTotal
              ? `${Math.round(downloaded / 1048576)} / ${Math.round(downloadTotal / 1048576)} MB`
              : `${Math.round(downloaded / 1048576)} MB`}
          </p>
        )}
        {(updatePhase === "installed" || updatePhase === "terminal") && (
          <button
            className="secondary"
            onClick={() => void api("restart_launcher")}
          >
            Restart Blockyard after installation
          </button>
        )}
        {update && (
          <button className="primary" disabled={updating} onClick={install}>
            <Download size={16} /> Install {update}
          </button>
        )}
      </div>
    </div>
  );
}
