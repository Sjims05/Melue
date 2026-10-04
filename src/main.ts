import { createApp } from "vue";
import "./style.css";
import { isMobile } from "./platform";
import { startPlayback } from "./library/playback";
import DesktopShell from "./shells/desktop/DesktopShell.vue";
import MobileShell from "./shells/mobile/MobileShell.vue";

// Phone and desktop have their own shell (navigation and layout) around the same pages.
// The platform is fixed at build time, so the other shell is left out of the build.
createApp(isMobile ? MobileShell : DesktopShell).mount("#app");

// The player's buttons, and counting what it plays (also what played while the app wasn't running).
startPlayback().catch((e) => console.error("melue: playback setup failed", e));
