import { useEffect, useRef, useState } from "react";
import type { MouseEvent } from "react";
import { marked } from "marked";
import DOMPurify from "dompurify";
import { openUrl } from "@tauri-apps/plugin-opener";
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
type ProjectDetail = {
  body: string;
  format: "markdown" | "html";
  iconUrl: string | null;
  gallery: string[];
  websiteUrl: string | null;
};
type Release = {
  id: string;
  name: string;
  minecraft: string[];
  loaders: string[];
  date: string;
};
function openExternal(raw: string, base?: string) {
  try {
    const url = new URL(raw, base);
    if (url.protocol === "https:") void openUrl(url.toString());
  } catch {
    /* Ignore malformed links from a project description. */
  }
}
export function CatalogView({
  instances,
  target,
  onInstalled,
  onSettings,
}: {
  instances: Instance[];
  target: Instance | undefined;
  onInstalled: (instance?: Instance) => Promise<void>;
  onSettings: () => void;
}) {
  const [provider, setProvider] = useState<Provider>("modrinth");
  const [kind, setKind] = useState<Kind>(target ? "mod" : "modpack");
  const [query, setQuery] = useState("");
  const [projects, setProjects] = useState<Project[]>([]);
  const [selected, setSelected] = useState<Project | null>(null);
  const [releases, setReleases] = useState<Release[]>([]);
  const [detail, setDetail] = useState<ProjectDetail | null>(null);
  const [keyConfigured, setKeyConfigured] = useState<boolean | null>(null);
  const detailRequest = useRef(0);
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
    setDetail(null);
    detailRequest.current += 1;
  }, [provider, kind]);
  useEffect(() => {
    if (provider === "curseforge")
      api<boolean>("catalog_key_status")
        .then(setKeyConfigured)
        .catch(() => setKeyConfigured(false));
  }, [provider]);
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
    const request = ++detailRequest.current;
    setSelected(project);
    setReleases([]);
    setDetail(null);
    setRelease("");
    setMessage("");
    setLoading(true);
    const [versionsResult, detailResult] = await Promise.allSettled([
      api<Release[]>("catalog_releases", {
        provider,
        project: project.id,
        instanceId: kind === "mod" ? instanceId || null : null,
      }),
      api<ProjectDetail>("catalog_project_detail", {
        provider,
        project: project.id,
      }),
    ]);
    if (request !== detailRequest.current) return;
    const errors: string[] = [];
    if (versionsResult.status === "fulfilled") {
      setReleases(versionsResult.value);
      setRelease(versionsResult.value[0]?.id || "");
      if (!versionsResult.value.length)
        errors.push(
          "No compatible files for this Minecraft version and loader.",
        );
    } else errors.push(String(versionsResult.reason));
    if (detailResult.status === "fulfilled") setDetail(detailResult.value);
    else
      errors.push(
        `Could not load project description: ${String(detailResult.reason)}`,
      );
    setMessage(errors.join(" "));
    setLoading(false);
  };
  const openDescriptionLink = (event: MouseEvent<HTMLDivElement>) => {
    const anchor = (event.target as HTMLElement).closest("a");
    if (!anchor) return;
    event.preventDefault();
    const raw = anchor.getAttribute("href");
    if (!raw) return;
    const base =
      provider === "modrinth"
        ? "https://modrinth.com"
        : "https://www.curseforge.com";
    openExternal(raw, base);
  };
  const descriptionHtml = detail
    ? DOMPurify.sanitize(
        detail.format === "markdown"
          ? (marked.parse(detail.body, { async: false }) as string)
          : detail.body,
        {
          USE_PROFILES: { html: true },
          FORBID_TAGS: [
            "iframe",
            "style",
            "form",
            "video",
            "audio",
            "object",
            "embed",
          ],
          FORBID_ATTR: ["style"],
        },
      ).replace(/(src|href)="\/\//g, '$1="https://')
    : "";
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
                setDetail(null);
                detailRequest.current += 1;
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
          disabled={
            loading ||
            (kind === "mod" && !instanceId) ||
            (provider === "curseforge" && !keyConfigured)
          }
        >
          <Search size={16} /> Search
        </button>
      </form>
      {provider === "curseforge" && keyConfigured === false && (
        <div className="settings-card catalog-key-notice">
          <h3>Connect CurseForge</h3>
          <p>
            CurseForge requires an official API key for catalog search and
            downloads. Add your key in Settings; it stays in your system
            keyring.
          </p>
          <div className="catalog-key-actions">
            <button className="primary" onClick={onSettings}>
              Open Settings
            </button>
            <button
              className="secondary"
              onClick={() =>
                void openUrl(
                  "https://support.curseforge.com/support/solutions/articles/9000208346-about-the-curseforge-api-and-how-to-apply-for-a-key",
                )
              }
            >
              How to get a key
            </button>
          </div>
        </div>
      )}
      {provider === "curseforge" && keyConfigured && (
        <p className="muted">
          Files whose authors disallow third party downloads cannot be installed
          here.
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
              <div className="catalog-detail-head">
                {(detail?.iconUrl || selected.iconUrl) && (
                  <img src={detail?.iconUrl || selected.iconUrl || ""} alt="" />
                )}
                <div>
                  <h3>{selected.title}</h3>
                  <p className="muted">{selected.description}</p>
                </div>
              </div>
              {detail?.websiteUrl && (
                <button
                  className="textbutton"
                  onClick={() => openExternal(detail.websiteUrl!)}
                >
                  View on {provider === "modrinth" ? "Modrinth" : "CurseForge"}{" "}
                  ↗
                </button>
              )}
              {detail?.body ? (
                <div
                  className="catalog-description"
                  onClick={openDescriptionLink}
                  dangerouslySetInnerHTML={{ __html: descriptionHtml }}
                />
              ) : loading ? (
                <p className="muted">Loading description…</p>
              ) : (
                <p className="muted">
                  No full description is available for this project.
                </p>
              )}
              {detail?.gallery.length ? (
                <div className="catalog-gallery">
                  {detail.gallery.map((url, index) => (
                    <img
                      key={`${url}-${index}`}
                      src={url}
                      alt={`${selected.title} screenshot ${index + 1}`}
                      loading="lazy"
                    />
                  ))}
                </div>
              ) : null}
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
              Select a project to view its full description and available
              versions.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
