import { useState } from "react";
import { Plus, Power, Copy, RefreshCw } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, Database, DeviceCode } from "../shared";
export function AccountsView({
  db,
  action,
  refresh,
}: {
  db: Database;
  action: (f: () => Promise<unknown>) => void;
  refresh: () => Promise<void>;
}) {
  const [code, setCode] = useState<DeviceCode | null>(null),
    [waiting, setWaiting] = useState(false),
    [showOffline, setShowOffline] = useState(false),
    [offlineName, setOfflineName] = useState("");
  const login = () =>
    action(async () => {
      const c = await api<DeviceCode>("start_login");
      setCode(c);
      setWaiting(true);
      await openUrl(c.verificationUri);
      api("finish_login", { code: c })
        .then(() => {
          setCode(null);
          setWaiting(false);
          refresh();
        })
        .catch((e) => {
          setWaiting(false);
          alert(String(e));
        });
    });
  return (
    <div className="content narrow">
      <div className="section-head">
        <div>
          <h2>Player profiles</h2>
          <p>Choose a local offline profile or sign in with Microsoft.</p>
        </div>
        <div className="account-actions">
          <button
            className="secondary"
            onClick={() => setShowOffline((v) => !v)}
          >
            <Plus size={16} /> Add offline profile
          </button>
          <button
            className="primary"
            onClick={login}
            disabled={!db.settings.microsoftClientId.trim()}
            title={
              !db.settings.microsoftClientId.trim()
                ? "Set Microsoft client ID in Settings first"
                : undefined
            }
          >
            <Plus size={16} /> Microsoft sign in
          </button>
        </div>
      </div>
      {showOffline && (
        <form
          className="offline-form"
          onSubmit={(e) => {
            e.preventDefault();
            action(() => api("add_offline_account", { name: offlineName }));
          }}
        >
          <label>
            Player name
            <input
              autoFocus
              value={offlineName}
              onChange={(e) => setOfflineName(e.target.value)}
              placeholder="Steve"
              minLength={3}
              maxLength={16}
              pattern="[A-Za-z0-9_]{3,16}"
              required
            />
          </label>
          <p>
            Offline profiles work for singleplayer and servers that allow
            offline players. Online servers require a Microsoft account.
          </p>
          <button className="primary" type="submit">
            Create offline profile
          </button>
        </form>
      )}
      {!db.settings.microsoftClientId.trim() && (
        <p className="account-note">
          Microsoft sign in needs a client ID in Settings. Offline profiles can
          be created now.
        </p>
      )}
      {code && (
        <div className="code-card">
          <div className="eyebrow">SIGN IN IN YOUR BROWSER</div>
          <h2>{code.userCode}</h2>
          <p>{code.message}</p>
          <button
            className="secondary"
            onClick={() => navigator.clipboard.writeText(code.userCode)}
          >
            <Copy size={16} /> Copy code
          </button>
          {waiting && (
            <span className="waiting">
              <RefreshCw size={14} /> Waiting for authorization
            </span>
          )}
        </div>
      )}
      <div className="list-card">
        {db.accounts.map((a) => (
          <div className="account-row" key={a.id}>
            <div className="avatar">{a.name[0]?.toUpperCase()}</div>
            <div>
              <strong>{a.name}</strong>
              <span>
                {a.kind === "offline"
                  ? "Offline profile · local play"
                  : "Microsoft · Minecraft Java profile"}
              </span>
            </div>
            {db.activeAccount === a.id ? (
              <span className="badge">Active</span>
            ) : (
              <button
                className="secondary compact"
                onClick={() =>
                  action(() => api("select_account", { id: a.id }))
                }
              >
                Use account
              </button>
            )}
            <button
              className="iconbutton"
              title={a.kind === "offline" ? "Remove profile" : "Sign out"}
              onClick={() => action(() => api("logout", { id: a.id }))}
            >
              <Power size={16} />
            </button>
          </div>
        ))}
        {!db.accounts.length && (
          <div className="empty">
            No profiles yet. Add an offline profile or connect a Microsoft
            account.
          </div>
        )}
      </div>
    </div>
  );
}
