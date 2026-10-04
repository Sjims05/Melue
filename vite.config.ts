import { defineConfig } from "vite";
import tailwindcss from "@tailwindcss/vite";
import vue from "@vitejs/plugin-vue";
// @ts-expect-error type error without @types/node package
import process from "node:process";
const host = process.env.TAURI_DEV_HOST;

// Set by `tauri dev` / `tauri build`, following the build target: android / androideabi, ios, windows,
// darwin (macOS), linux. Missing when Vite runs on its own; the app then acts as desktop.
const platform = process.env.TAURI_ENV_PLATFORM ?? "";
const isAndroid = platform === "android" || platform === "androideabi";
const isIos = platform === "ios";

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [vue(), tailwindcss()],

  // The platform this build is for, as true / false constants (see src/platform/platform.ts). Being plain
  // constants, Vite drops the code for the other platforms from the build.
  define: {
    __IS_ANDROID__: JSON.stringify(isAndroid),
    __IS_IOS__: JSON.stringify(isIos),
  },

  // tauri-plugin-native-audio-api is linked from a sibling folder, so Vite resolves its
  // imports from there (where there is no node_modules). Resolve @tauri-apps/api from
  // this project instead, which also guarantees a single copy.
  resolve: {
    dedupe: ["@tauri-apps/api"],
    // Import aliases (@/...) from tsconfig.json's paths.
    tsconfigPaths: true,
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
