<script lang="ts">
import { isTauri } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { ref } from 'vue'
import { isDesktop } from '@/platform'

// Whether the window is maximized, shared by every top bar's buttons (one listener for the whole app). The window is
// asked only once it has stopped resizing: asked while it's being maximized (Windows), the answer waits on the
// window's own thread, which can be busy with the resize, and the app can hang. Only on a computer: the phone loads this
// file too (main.ts imports both frames), and a phone's window has no such questions (Tauri rejects them).
const maximized = ref(false)
if (isTauri() && isDesktop) {
  const win = getCurrentWindow()
  const ask = () => win.isMaximized().then((m) => (maximized.value = m), () => {})
  let settle: ReturnType<typeof setTimeout> | undefined
  ask()
  win.onResized(() => {
    clearTimeout(settle)
    settle = setTimeout(ask, 200)
  })
}
</script>

<script setup lang="ts">
/**
 * Melue's own window buttons (the window has no system frame: tauri.windows.conf.json): minimize, maximize / restore
 * (its icon follows the window), close (which, like the system's, follows "Closing the window": the tray, or quit).
 * Not shown in a plain browser (the dev preview).
 */
import { Copy, Minus, Square, X } from '@lucide/vue'

const inApp = isTauri()
// Maximize / restore: the icon turns at once; the window's answer follows once it has settled.
function toggleMaximize() {
  maximized.value = !maximized.value
  getCurrentWindow().toggleMaximize()
}

const BUTTON = 'grid size-10 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-card hover:text-foreground [&_svg]:size-4'
</script>

<template>
  <div v-if="inApp" class="flex items-center gap-1">
    <button type="button" :class="BUTTON" aria-label="Minimize" @click="getCurrentWindow().minimize()"><Minus /></button>
    <button type="button" :class="BUTTON" :aria-label="maximized ? 'Restore' : 'Maximize'" @click="toggleMaximize">
      <Copy v-if="maximized" class="size-3.5! -scale-x-100" />
      <Square v-else class="size-3.5!" />
    </button>
    <button type="button" :class="[BUTTON, 'hover:bg-destructive hover:text-white']" aria-label="Close" @click="getCurrentWindow().close()"><X /></button>
  </div>
</template>
