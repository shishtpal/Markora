const FONT_STACKS = {
  "space-grotesk": '"Space Grotesk", "Noto Sans Devanagari", sans-serif',
  system: '"Segoe UI", "Noto Sans Devanagari", system-ui, sans-serif',
  georgia: 'Georgia, "Times New Roman", "Noto Sans Devanagari", serif',
  arial: 'Arial, "Noto Sans Devanagari", sans-serif',
  "ibm-plex-mono": '"IBM Plex Mono", "Noto Sans Devanagari", monospace',
  consolas: 'Consolas, "IBM Plex Mono", "Noto Sans Devanagari", monospace',
};

// Use an allowlist shared with Rust preferences, never saved CSS from user input.
export function applyTypography(prefs) {
  const root = document.documentElement;
  const family = FONT_STACKS[prefs.fontFamily];
  root.toggleAttribute("data-custom-font", Boolean(family));
  if (family) root.style.setProperty("--editor-font", family);
  else root.style.removeProperty("--editor-font");
  root.style.setProperty("--editor-size", `${prefs.fontSize ?? 16}px`);
  const themed = prefs.mdTheme === "paper" ? FONT_STACKS.georgia
    : prefs.mdTheme === "mono" ? FONT_STACKS["ibm-plex-mono"]
    : prefs.mdTheme === "github" ? FONT_STACKS.system : "var(--font-ui)";
  root.style.setProperty("--preview-font", family || themed);
}
