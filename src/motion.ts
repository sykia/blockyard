const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");

export function applyMotion(enabled: boolean) {
  document.documentElement.dataset.motionPreference = enabled ? "on" : "off";
  document.documentElement.dataset.motion =
    enabled && !reducedMotion.matches ? "on" : "off";
}

try {
  applyMotion(localStorage.getItem("blockyard-animations") !== "off");
} catch {
  applyMotion(true);
}

reducedMotion.addEventListener("change", () => {
  applyMotion(document.documentElement.dataset.motionPreference !== "off");
});
