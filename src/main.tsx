import React, { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { listen } from "@tauri-apps/api/event";
import { openPath } from "@tauri-apps/plugin-opener";
import {
  Box,
  Play,
  Plus,
  Settings as SettingsIcon,
  Users,
  ScrollText,
  X,
  ChevronRight,
  Puzzle,
  Compass,
} from "lucide-react";
import {
  api,
  Database,
  Version,
  Status,
  Instance,
  InstanceGlyph,
} from "./shared";
import { CreateModal } from "./views/CreateModal";
import { InstanceDetail } from "./views/InstanceDetail";
import { ModsView } from "./views/ModsView";
import { CatalogView } from "./views/CatalogView";
import { AccountsView } from "./views/AccountsView";
import { SettingsView } from "./views/SettingsView";
import { LogsView } from "./views/LogsView";
import "./style.css";

function App() {
  const [db, setDb] = useState<Database | null>(null),
    [versions, setVersions] = useState<Version[]>([]),
    [page, setPage] = useState("home"),
    [selected, setSelected] = useState<string | null>(null),
    [status, setStatus] = useState<Status | null>(null),
    [lines, setLines] = useState<string[]>([]),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false),
    [create, setCreate] = useState(false),
    [catalogTarget, setCatalogTarget] = useState<Instance | undefined>(
      undefined,
    );
  const refresh = async () => {
    try {
      const next = await api<Database>("snapshot");
      setDb(next);
      if (next.warnings.length) setError(next.warnings.join(" "));
    } catch (e) {
      setError(String(e));
    }
  };
  useEffect(() => {
    refresh();
    const un1 = listen<Status>("game-status", (e) => {
      setStatus(e.payload);
      if (e.payload.phase === "error" || e.payload.phase === "crashed")
        setPage("logs");
    });
    const un2 = listen<{ instanceId: string; line: string }>("game-log", (e) =>
      setLines((v) => [...v.slice(-499), e.payload.line]),
    );
    return () => {
      un1.then((f) => f());
      un2.then((f) => f());
    };
  }, []);
  useEffect(() => {
    if (db)
      api<Version[]>("versions")
        .then(setVersions)
        .catch((e) => setError(String(e)));
  }, [db?.settings.showSnapshots]);
  const action = async (fn: () => Promise<unknown>) => {
    setError("");
    setBusy(true);
    try {
      await fn();
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const current =
    db?.instances.find((i) => i.id === selected) || db?.instances[0];
  const account = db?.accounts.find((a) => a.id === db.activeAccount);
  const play = () =>
    current &&
    action(async () => {
      setLines([]);
      await api("play", { id: current.id });
    });
  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brandmark">
            <Box size={23} />
          </div>
          <span>Blockyard</span>
        </div>
        <nav>
          {[
            ["home", "Overview", Box],
            ["instances", "Instances", Puzzle],
            ["discover", "Discover", Compass],
            ["accounts", "Accounts", Users],
            ["logs", "Logs", ScrollText],
            ["settings", "Settings", SettingsIcon],
          ].map(([key, label, Icon]) => (
            <button
              key={key as string}
              className={page === key ? "active" : ""}
              onClick={() => {
                if (key === "discover") setCatalogTarget(undefined);
                setPage(key as string);
              }}
            >
              <Icon size={18} />
              {label as string}
            </button>
          ))}
        </nav>
        <div className="sidebar-foot">
          <span className="signal" /> Minecraft Java Edition{" "}
          <small>v0.3.0</small>
        </div>
      </aside>
      <main>
        <header>
          <div className="eyebrow">YOUR MINECRAFT SPACE</div>
          <div className="header-row">
            <h1>
              {
                (
                  {
                    home: "Overview",
                    instances: "Instances",
                    accounts: "Accounts",
                    logs: "Logs & crashes",
                    settings: "Settings",
                    mods: "Mods",
                    discover: "Discover",
                  } as Record<string, string>
                )[page]
              }
            </h1>
            <div className="header-actions">
              {page === "instances" && (
                <button className="primary" onClick={() => setCreate(true)}>
                  <Plus size={17} /> New instance
                </button>
              )}
              {page === "home" && (
                <button
                  className="primary"
                  onClick={() => {
                    setPage("instances");
                    setCreate(true);
                  }}
                >
                  <Plus size={17} /> New instance
                </button>
              )}
            </div>
          </div>
        </header>
        {error && (
          <div className="error">
            <span>{error}</span>
            <button onClick={() => setError("")}>
              <X size={16} />
            </button>
          </div>
        )}
        {page === "home" && (
          <div className="content">
            <div className="hero">
              <div>
                <div className="eyebrow">READY WHEN YOU ARE</div>
                <h2>{current ? current.name : "Create your first instance"}</h2>
                <p>
                  {current
                    ? `${current.version} · ${current.loader.kind === "vanilla" ? "Vanilla" : current.loader.kind === "fabric" ? "Fabric" : "NeoForge"}`
                    : "Keep every world, mod and setting in its own space."}
                </p>
                <div className="hero-actions">
                  <button
                    className="play"
                    disabled={!current || busy || status?.phase === "running"}
                    onClick={play}
                  >
                    <Play size={18} fill="currentColor" />
                    {status?.phase === "running" &&
                    status.instanceId === current?.id
                      ? "Running"
                      : "Play Minecraft"}
                  </button>
                  <button
                    className="secondary"
                    onClick={() => setPage("accounts")}
                  >
                    {account ? account.name : "Connect account"}{" "}
                    <ChevronRight size={16} />
                  </button>
                </div>
              </div>
              <div className="hero-symbol">
                <Box size={100} strokeWidth={1} />
              </div>
            </div>
            {status && (
              <div className="progress-card">
                <div>
                  <strong>
                    {status.phase === "crashed"
                      ? "Minecraft stopped unexpectedly"
                      : status.phase === "error"
                        ? "Launch failed"
                        : status.phase === "running"
                          ? "Minecraft is running"
                          : status.phase === "stopped"
                            ? "Session ended"
                            : `Preparing ${status.phase}`}
                  </strong>
                  <p>{status.message}</p>
                </div>
                {["libraries", "assets", "install"].includes(status.phase) && (
                  <div className="track">
                    <div
                      style={{ width: `${Math.round(status.progress * 100)}%` }}
                    />
                  </div>
                )}
              </div>
            )}
            <div className="section-head">
              <div>
                <h3>Your instances</h3>
                <p>Independent worlds, mods and settings</p>
              </div>
              <button
                className="textbutton"
                onClick={() => setPage("instances")}
              >
                View all <ChevronRight size={16} />
              </button>
            </div>
            <div className="instance-grid">
              {db?.instances.map((i) => (
                <button
                  className={
                    "instance-card " + (i.id === current?.id ? "chosen" : "")
                  }
                  key={i.id}
                  onClick={() => setSelected(i.id)}
                >
                  <div className="instance-icon">
                    <InstanceGlyph icon={i.icon} />
                  </div>
                  <strong>{i.name}</strong>
                  <span>
                    {i.version} · {i.loader.kind}
                  </span>
                  <small>
                    {i.lastPlayed
                      ? `Played ${new Date(i.lastPlayed).toLocaleDateString()}`
                      : "Never played"}
                  </small>
                </button>
              ))}
              {!db?.instances.length && (
                <div className="empty">
                  No instances yet. Create one to get started.
                </div>
              )}
            </div>
          </div>
        )}
        {page === "instances" && (
          <div className="content split">
            <div className="instance-list">
              <div className="list-title">
                ALL INSTANCES <span>{db?.instances.length || 0}</span>
              </div>
              {db?.instances.map((i) => (
                <button
                  key={i.id}
                  className={
                    "instance-row " + (i.id === current?.id ? "selected" : "")
                  }
                  onClick={() => setSelected(i.id)}
                >
                  <div className="instance-icon small">
                    <InstanceGlyph icon={i.icon} />
                  </div>
                  <div>
                    <strong>{i.name}</strong>
                    <span>
                      {i.version} · {i.loader.kind}
                    </span>
                  </div>
                  <ChevronRight size={16} />
                </button>
              ))}
              {!db?.instances.length && (
                <div className="empty">Create an instance to begin.</div>
              )}
            </div>
            <div className="detail">
              {current ? (
                <InstanceDetail
                  instance={current}
                  busy={busy}
                  onSave={(i) =>
                    action(() => api("save_instance", { instance: i }))
                  }
                  onDelete={() =>
                    action(async () => {
                      await api("delete_instance", { id: current.id });
                      setSelected(null);
                    })
                  }
                  onPlay={play}
                  onMods={() => setPage("mods")}
                  onLogs={() => setPage("logs")}
                  onOpen={() =>
                    action(async () =>
                      openPath(
                        await api<string>("instance_directory", {
                          id: current.id,
                        }),
                      ),
                    )
                  }
                />
              ) : (
                <div className="empty">Select an instance</div>
              )}
            </div>
          </div>
        )}
        {page === "discover" && (
          <CatalogView
            instances={db?.instances || []}
            target={catalogTarget}
            onInstalled={async (i) => {
              await refresh();
              if (i) setSelected(i.id);
            }}
          />
        )}
        {page === "mods" && current && (
          <ModsView
            instance={current}
            action={action}
            onBack={() => setPage("instances")}
            onDiscover={() => {
              setCatalogTarget(current);
              setPage("discover");
            }}
          />
        )}
        {page === "accounts" && db && (
          <AccountsView db={db} action={action} refresh={refresh} />
        )}
        {page === "settings" && db && (
          <SettingsView
            settings={db.settings}
            onSave={(s) => action(() => api("save_settings", { settings: s }))}
          />
        )}
        {page === "logs" && (
          <LogsView
            instance={current}
            status={status}
            lines={lines}
            action={action}
          />
        )}
      </main>
      {create && (
        <CreateModal
          versions={versions}
          onClose={() => setCreate(false)}
          onCreate={(name, version, loader) =>
            action(async () => {
              const i = await api<Instance>("create_instance", {
                name,
                version,
                loader,
              });
              setSelected(i.id);
              setCreate(false);
              setPage("instances");
            })
          }
        />
      )}
    </div>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
