/// <reference types="vite/client" />

declare module "*.vue" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<{}, {}, any>;
  export default component;
}

/** Whether this build is for Android / iOS (set in vite.config.ts from TAURI_ENV_PLATFORM). */
declare const __IS_ANDROID__: boolean;
declare const __IS_IOS__: boolean;
