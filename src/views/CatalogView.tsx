import { useEffect, useState } from "react";
import { Download, Search } from "lucide-react";
import { api, Instance } from "../shared";

type Provider = "modrinth" | "curseforge";
type Kind = "mod" | "modpack";
type Project = {
  id: string;
  title: string;
  description: string;
  iconUrl: string | null;
  downloads: number;
  provider: Provider;
};
type Release = {
  id: string;
  name: string;
  minecraft: string[];
  loaders: string[];
  date: string;
};
export function CatalogView({
  instances,
  target,
  onInstalled,
}: {
  instances: Instance[];
  target: Instance | undefined;
  onInstalled: (instance?: Instance) => Promise<void>;
}) {
  const [provider, setProvider] = useState<Provider>("modrinth");
  const [kind, setKind] = useState<Kind>(target ? "mod" : "modpack");
  const [query, setQuery] = useState("");
  const [projects, setProjects] = useState<Project[]>([]);
  const [selected, setSelected] = useState<Project | null>(null);
  const [releases, setReleases] = useState<Release[]>([]);
  const [release, setRelease] = useState("");
  const [instanceId, setInstanceId] = useState(
    target?.id || instances.find((i) => i.loader.kind !== "vanilla")?.id || "",
  );
  const [loading, setLoading] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [message, setMessage] = useState("");
  useEffect(() => {
    setSelected(null);
    setProjects([]);
    setReleases([]);
  }, [provider, kind]);
  const search = async () => {
    setLoading(true);
    setMessage("");
    setSelected(null);
    try {
      const results = await api<Project[]>("catalog_search", {
        provider,
        kind,
        query,
        instanceId: kind === "mod" ? instanceId || null : null,
      });
      setProjects(results);
      if (!results.length) setMessage("No matching projects found.");
    } catch (e) {
      setMessage(String(e));
    } finally {
      setLoading(false);
    }
  };
  const choose = async (project: Project) => {
    setSelected(project);
    setReleases([]);
    setRelease("");
    setMessage("");
    setLoading(true);
    try {
      const list = await api<Release[]>("catalog_releases", {
        provider,
        project: project.id,
        instanceId: kind === "mod" ? instanceId || null : null,
      });
      setReleases(list);
      setRelease(list[0]?.id || "");
      if (!list.length)
        setMessage(
          "No compatible files for this Minecraft version and loader.",
        );
    } catch (e) {
      setMessage(String(e));
    } finally {
      setLoading(false);
    }
  };
  const install = async () => {
    if (!selected || !release) return;
    setInstalling(true);
    setMessage(
      kind === "modpack"
        ? "Installing pack and its files…"
        : "Downloading mod and required dependencies…",
    );
    try {
      if (kind === "mod") {
        if (!instanceId) throw new Error("Select an instance first.");
        await api("catalog_install_mod", {
          provider,
          project: selected.id,
          release,
          instanceId,
        });
        await onInstalled();
        setMessage("Mod installed. Check the Mods screen before launching.");
      } else {
        const instance = await api<Instance>("catalog_install_pack", {
          provider,
          project: selected.id,
          release,
        });
        await onInstalled(instance);
        setMessage(
          `Created ${instance.name}. Select it from Instances to play.`,
        );
      }
    } catch (e) {
      setMessage(String(e));
    } finally {
      setInstalling(false);
    }
  };
  return (
    <div className="content catalog-page">
      <div className="section-head">
        <div>
          <h2>Discover</h2>
          <p>Find a mod or create an independent instance from a modpack.</p>
        </div>
      </div>
      <div className="catalog-controls">
        <label>
          Source
          <select
            value={provider}
            onChange={(e) => setProvider(e.target.value as Provider)}
          >
            <option value="modrinth">Modrinth</option>
            <option value="curseforge">CurseForge</option>
          </select>
        </label>
        <label>
          Content
          <select
            value={kind}
            onChange={(e) => setKind(e.target.value as Kind)}
          >
            <option value="mod">Mods</option>
            <option value="modpack">Modpacks</option>
          </select>
        </label>
        {kind === "mod" && (
          <label>
            Instance
            <select
              value={instanceId}
              onChange={(e) => {
                setInstanceId(e.target.value);
                setProjects([]);
                setSelected(null);
                setReleases([]);
              }}
            >
              {instances
                .filter((i) => i.loader.kind !== "vanilla")
                .map((i) => (
                  <option key={i.id} value={i.id}>
                    {i.name} · {i.version} · {i.loader.kind}
                  </option>
                ))}
            </select>
          </label>
        )}
      </div>
      {kind === "mod" &&
        !instances.some((i) => i.loader.kind !== "vanilla") && (
          <p className="muted">
            Create a Fabric or NeoForge instance before installing mods.
          </p>
        )}
      <form
        className="catalog-search"
        onSubmit={(e) => {
          e.preventDefault();
          void search();
        }}
      >
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search projects"
        />
        <button
          className="primary"
          disabled={loading || (kind === "mod" && !instanceId)}
        >
          <Search size={16} /> Search
        </button>
      </form>
      {provider === "curseforge" && (
        <p className="muted">
          CurseForge requires your own API key in Settings. Files whose authors
          disallow third party downloads cannot be installed here.
        </p>
      )}
      {message && <p className="catalog-message">{message}</p>}
      <div className="catalog-layout">
        <div className="list-card catalog-results">
          {projects.map((p) => (
            <button
              key={p.id}
              className={
                selected?.id === p.id
                  ? "catalog-project selected"
                  : "catalog-project"
              }
              onClick={() => void choose(p)}
            >
              {p.iconUrl ? (
                <img src={p.iconUrl} alt="" />
              ) : (
                <span className="catalog-placeholder" />
              )}
              <span>
                <strong>{p.title}</strong>
                <small>{p.description}</small>
                <small>
                  {Intl.NumberFormat().format(p.downloads)} downloads
                </small>
              </span>
            </button>
          ))}
          {loading && !projects.length && <div className="empty">Loading…</div>}
        </div>
        <div className="settings-card catalog-detail">
          {selected ? (
            <>
              <h3>{selected.title}</h3>
              <p className="muted">{selected.description}</p>
              <label>
                Version
                <select
                  value={release}
                  onChange={(e) => setRelease(e.target.value)}
                >
                  {releases.map((r) => (
                    <option key={r.id} value={r.id}>
                      {r.name}{" "}
                      {r.minecraft.length
                        ? `· ${r.minecraft.slice(0, 3).join(", ")}`
                        : ""}
                    </option>
                  ))}
                </select>
              </label>
              <button
                className="primary"
                disabled={!release || installing}
                onClick={() => void install()}
              >
                <Download size={16} />
                {installing
                  ? "Installing…"
                  : kind === "modpack"
                    ? "Install as new instance"
                    : "Install mod"}
              </button>
            </>
          ) : (
            <p className="muted">
              Select a project to view available versions.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
