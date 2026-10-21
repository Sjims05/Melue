/**
 * Links to Melue from outside it (Tauri's deep-link plugin; the scheme is in tauri.conf.json): melue://settings/<id>
 * opens Settings at that setting (a setting's Copy link). Melue started by a link opens at it; a running Melue is
 * brought to the front (desktop.rs's one-Melue-at-a-time) and goes there.
 */
import { getCurrent, onOpenUrl } from '@tauri-apps/plugin-deep-link'
import type { Router } from 'vue-router'

function open(router: Router, urls: string[] | null) {
  for (const url of urls ?? []) {
    const setting = /^melue:\/\/settings\/([\w.-]+)/.exec(url)?.[1]
    if (setting) return void router.push({ path: '/settings', query: { setting } })
    console.warn('melue: a link Melue doesn’t know', url)
  }
}

/** Once at start (in the app, not a browser). */
export async function followLinks(router: Router) {
  open(router, await getCurrent().catch(() => null))
  await onOpenUrl((urls) => open(router, urls))
}
