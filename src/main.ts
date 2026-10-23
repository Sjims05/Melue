import { createApp } from "vue";
import { getVersion } from "@tauri-apps/api/app";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { Sparkles } from "@lucide/vue";
import { toast } from "./lib/toast";
import { updatedSinceLastRun } from "./lib/updatedNote";
import "./style.css";
// Light or dark (follows the system unless set).
import "./lib/theme";
import { isMobile } from "./platform";
import { startPlayback } from "./library/playback";
import { syncNowPlaying } from "./player/nowPlaying";
import { startSync } from "./library/sync";
import DesktopShell from "./shells/desktop/DesktopShell.vue";
import MobileShell from "./shells/mobile/MobileShell.vue";
import { devBuild, readDevBuild } from "./lib/devBuild";
import { startTrail } from "./lib/report/trail";
import { appearance } from "./lib/appearance";
import { offerCopy } from "./lib/copyMenu";
import { startHoverTips } from "./lib/hoverTips";
import { addDevPage, preloadPages, router, startPage } from "./router";

// Hover tips in Melue's style (every title: lib/hoverTips.ts).
startHoverTips();

// No browser right-click menu: Melue's own menus (e.g. a setting's "Reset to default") open where there is one; on an
// error, Copy (lib/copyMenu.ts). Text fields keep the browser's, for copy and paste.
document.addEventListener("contextmenu", (e) => {
  if ((e.target as Element | null)?.closest?.("input, textarea, [contenteditable]")) return;
  const taken = e.defaultPrevented;
  e.preventDefault();
  if (!taken) offerCopy(e, isMobile);
});

async function start(shell = isMobile ? MobileShell : DesktopShell) {
  // Opening without a page asked for: the one Start on says (Display settings), there before the app mounts (mounting
  // takes the router to Home otherwise).
  // Melue Dev: its dev page (lib/devBuild.ts), there before the app is drawn.
  await readDevBuild();
  if (devBuild.value) {
    addDevPage();
    import("./dev/testHooks").then(({ installTestHooks }) => installTestHooks()).catch(() => {});
  }
  const page = startPage(appearance.startOn);
  if ((!location.hash || location.hash === "#/") && page !== "/") await router.replace(page).catch(() => {});
  // Phone and desktop have their own shell (navigation and layout) around the same pages.
  // The platform is fixed at build time, so the other shell is left out of the build.
  createApp(shell).use(router).mount("#app");
  preloadPages();
  // The app has drawn itself: the desktop window waits for this, hidden (no white window first), and so does
  // background work (startup.rs). A hidden window may not get animation frames, so a timer too.
  if (isTauri()) {
    let told = false;
    const ready = () => !told && (told = true) && invoke("window_ready", { pageMs: performance.now() }).catch(() => {});
    requestAnimationFrame(() => requestAnimationFrame(ready));
    setTimeout(ready, 100);
  }

  // The player's buttons, and counting what it plays (also what played while the app wasn't running).
  // Then what's playing is read again: the player may have resumed the last queue meanwhile.
  startPlayback()
    .catch((e) => console.error("melue: playback setup failed", e))
    .then(() => syncNowPlaying());
  // Links to Melue from outside it (melue://settings/<id>).
  if (isTauri()) import("./lib/links").then(({ followLinks }) => followLinks(router)).catch(() => {});
  // Updated since it last ran: a note, with What's new at hand (lib/updatedNote.ts).
  if (isTauri()) {
    getVersion().then((version) => {
      if (updatedSinceLastRun(version)) {
        toast(`Updated to Melue ${version}`, Sparkles, { label: "What's new", run: () => router.push({ path: "/settings", query: { whatsNew: "1" } }) });
      }
    }).catch(() => {});
  }
  // Updating itself from the newest published release (lib/updates.ts).
  if (isTauri()) import("./lib/updates").then(({ followUpdates }) => followUpdates()).catch(() => {});
  // What a bug report says was done before it: the last steps and errors (lib/report/trail.ts).
  startTrail();
  // Connect device: playing on another of your devices (player/remotePlayback.ts).
  if (isTauri()) import("./player/remotePlayback").then(({ startRemotePlayback }) => startRemotePlayback()).catch(() => {});
  // Whether a share is set up (sync buttons show only then), kept live.
  startSync().catch((e) => console.error("melue: sync status failed", e));
}

// In a dev browser (no Melue behind it), sample answers first: see dev/browserPreview.ts. Left out of the build.
if (import.meta.env.DEV && !isTauri()) {
  import("./dev/browserPreview").then(({ installBrowserPreview }) => {
    installBrowserPreview();
    // ?phone: the phone's frame, to look at it in the browser.
    start(new URLSearchParams(location.search).has("phone") ? MobileShell : undefined);
  });
} else {
  start();
}
