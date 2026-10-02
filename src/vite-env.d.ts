/// <reference types="vite/client" />

declare module "*.vue" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<{}, {}, any>;
  export default component;
}

interface ImportMetaEnv {
  readonly VITE_SMB_HOST: string;
  readonly VITE_SMB_USER: string;
  readonly VITE_SMB_PASS: string;
  readonly VITE_SMB_SHARE: string;
  readonly VITE_SMB_MUSIC_PATH: string;
}

/** Whether this build is for Android / iOS (set in vite.config.ts from TAURI_ENV_PLATFORM). */
declare const __IS_ANDROID__: boolean;
declare const __IS_IOS__: boolean;

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
