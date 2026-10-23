/**
 * Whether this is Melue Dev: a development build, or a phone's test build (src-tauri `app_is_dev`). Known before the
 * app is drawn (main.ts): only then is there the dev page (its tests and tools). In a browser preview: always.
 */
import { invoke, isTauri } from '@tauri-apps/api/core'
import { ref } from 'vue'

export const devBuild = ref(import.meta.env.DEV && !isTauri())

export async function readDevBuild() {
  if (isTauri()) devBuild.value = await invoke<boolean>('app_is_dev').catch(() => false)
}
