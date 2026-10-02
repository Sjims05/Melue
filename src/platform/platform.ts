/**
 * Which platform this build is for. Decided when the app is built (vite.config.ts turns the Tauri CLI's
 * TAURI_ENV_PLATFORM into true / false constants), so Vite leaves the other platforms' code out of the bundle.
 * When Vite runs on its own (`npm run dev` in a browser) the app acts as desktop.
 */
export const isAndroid = __IS_ANDROID__
export const isIos = __IS_IOS__
export const isMobile = __IS_ANDROID__ || __IS_IOS__
export const isDesktop = !isMobile
