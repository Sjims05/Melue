import { createApp } from "vue";
import { isTauri } from "@tauri-apps/api/core";
import "./style.css";
// Light or dark (follows the system unless set).
import "./lib/theme";
import { isMobile } from "./platform";
import { startPlayback } from "./library/playback";
import { startSync } from "./library/sync";
import DesktopShell from "./shells/desktop/DesktopShell.vue";
import MobileShell from "./shells/mobile/MobileShell.vue";

// No browser right-click menu: Melue's own menus (e.g. a setting's "Reset to default") open where there is one.
// Text fields keep it, for copy and paste.
document.addEventListener("contextmenu", (e) => {
  if (!(e.target as Element | null)?.closest?.("input, textarea, [contenteditable]")) e.preventDefault();
});

function start() {
  // Phone and desktop have their own shell (navigation and layout) around the same pages.
  // The platform is fixed at build time, so the other shell is left out of the build.
  createApp(isMobile ? MobileShell : DesktopShell).mount("#app");

  // The player's buttons, and counting what it plays (also what played while the app wasn't running).
  startPlayback().catch((e) => console.error("melue: playback setup failed", e));
  // Whether a share is set up (sync buttons show only then), kept live.
  startSync().catch((e) => console.error("melue: sync status failed", e));
}

// In a dev browser (no Melue behind it), sample answers first: see spike/browserPreview.ts. Left out of the build.
if (import.meta.env.DEV && !isTauri()) {
  import("./spike/browserPreview").then(({ installBrowserPreview }) => {
    installBrowserPreview();
    start();
  });
} else {
  start();
}
