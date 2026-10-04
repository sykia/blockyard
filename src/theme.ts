export type Theme = "dark" | "light" | "system";

const media = window.matchMedia("(prefers-color-scheme: dark)");

export function normalizeTheme(value: unknown): Theme {
  return value === "light" || value === "system" ? value : "dark";
}

export function applyTheme(choice: Theme) {
  document.documentElement.dataset.themePreference = choice;
  document.documentElement.dataset.theme =
    choice === "system" ? (media.matches ? "dark" : "light") : choice;
}

try {
  applyTheme(normalizeTheme(localStorage.getItem("blockyard-theme")));
} catch {
  applyTheme("dark");
}

media.addEventListener("change", () => {
  if (document.documentElement.dataset.themePreference === "system") {
    applyTheme("system");
  }
});
