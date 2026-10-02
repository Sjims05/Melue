import { createApp } from "vue";
import "./style.css";
import { isMobile } from "./platform";
import DesktopShell from "./shells/desktop/DesktopShell.vue";
import MobileShell from "./shells/mobile/MobileShell.vue";

// Phone and desktop have their own shell (navigation and layout) around the same pages.
// The platform is fixed at build time, so the other shell is left out of the build.
createApp(isMobile ? MobileShell : DesktopShell).mount("#app");
