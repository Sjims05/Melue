/**
 * Updating Melue itself (desktop; Tauri's updater: src-tauri tauri.conf.json `plugins.updater`): it looks at the newest
 * stable release on GitHub (its latest.json, signed: only an update signed with Melue's key installs) a minute after
 * start and every 6 hours. A newer one: Melue asks (UpdateDialog.vue: Don't ask again / Not now / Update), unless
 * "Ask when an update is found" is off (then About's Updates row offers it). Update downloads it, installs it
 * (Windows' installer, without questions) and starts Melue again; the new version then says so (lib/updatedNote.ts).
 *
 * On a phone (src-tauri phone_update.rs) the release's APK is downloaded and handed to Android's installer, which asks
 * and installs it (only when it's signed like the installed app); the first time, Android wants Melue allowed to install
 * apps (its screen opens; then Update again). The question is a drawer there.
 *
 * A development build goes through all of it without updating: the look finds a made-up next version, the download
 * and the install only seem to happen, and instead of starting again it shows the note a new version would.
 */
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { CircleSlash, Clock, Download, Sparkles } from '@lucide/vue'
import { reactive } from 'vue'
import { library } from '@/library'
import { isMobile } from '@/platform'
import { router } from '@/router'
import { openMenu } from './menuDrawer'
import { toast } from './toast'

type Update = NonNullable<Awaited<ReturnType<typeof import('@tauri-apps/plugin-updater').check>>>

/** A development build pretends (see above). */
const pretend = import.meta.env.DEV
const ASK_SETTING = 'update_ask'

export const updates = reactive({
  /** "idle" (not looked yet), "checking", "latest", "available", "downloading", "installing", "failed". */
  state: 'idle' as 'idle' | 'checking' | 'latest' | 'available' | 'downloading' | 'installing' | 'failed',
  /** The newer version, when there is one, and its notes. */
  version: '',
  notes: '',
  /** Downloaded, 0 to 1 (null: unknown size). */
  progress: null as number | null,
  error: '',
  /** When it last looked (ms). */
  checkedAt: 0,
  /** The dialog asking about it is open. */
  asking: false,
  /** "Ask when an update is found" (on by default; this device's own). */
  ask: true,
})

let found: Update | null = null
/** A phone's: the newest release's APK (phone_update.rs), and whether it was downloaded (allowing installs was asked). */
let apk: { url: string; size: number } | null = null
let downloaded = false

/** Whether version `a` ("0.6.0") is newer than `b`. */
const newer = (a: string, b: string) => {
  const [x, y] = [a, b].map((v) => v.split('.').map(Number))
  for (let i = 0; i < 3; i++) if ((x[i] ?? 0) !== (y[i] ?? 0)) return (x[i] ?? 0) > (y[i] ?? 0)
  return false
}
const FIRST_MS = 60_000
const EVERY_MS = 6 * 3_600_000
const wait = (ms: number) => new Promise((done) => setTimeout(done, ms))

/** The version after this one (the made-up update of a development build). */
async function nextVersion(): Promise<string> {
  const { getVersion } = await import('@tauri-apps/api/app')
  const [major, minor, patch] = (await getVersion()).split('.').map(Number)
  return `${major}.${minor}.${patch + 1}`
}

/** Looks for a newer release now. `quiet`: no note when it's already the latest (a look by itself). */
export async function checkForUpdate(quiet = false) {
  if (updates.state === 'downloading' || updates.state === 'installing') return
  updates.state = 'checking'
  updates.error = ''
  try {
    let version: string | null = null
    if (pretend) {
      await wait(800)
      version = await nextVersion()
      updates.notes = 'The changes in this version, as its release notes say them.'
    } else if (isMobile) {
      const release = await invoke<{ version: string; notes: string; url: string; size: number } | null>('update_latest_release')
      const { getVersion } = await import('@tauri-apps/api/app')
      const here = await getVersion()
      if (release && newer(release.version, here)) {
        version = release.version
        updates.notes = release.notes
        if (apk?.url !== release.url) downloaded = false
        apk = { url: release.url, size: release.size }
      }
    } else {
      const { check } = await import('@tauri-apps/plugin-updater')
      found = await check()
      version = found?.version ?? null
      updates.notes = found?.body ?? ''
    }
    updates.checkedAt = Date.now()
    if (version) {
      updates.state = 'available'
      updates.version = version
      if (updates.ask) askNow()
    } else {
      updates.state = 'latest'
      if (!quiet) toast('Melue is up to date')
    }
  } catch (e) {
    updates.state = 'failed'
    // No latest.json to read: offline, or no release published yet.
    updates.error = /valid release json|fetch/i.test(String(e)) ? 'Melue’s releases can’t be reached (offline, or none published yet)' : String(e)
  }
}

/** Asks about the update: the dialog (UpdateDialog.vue), or on a phone the menu drawer. */
function askNow() {
  if (!isMobile) return void (updates.asking = true)
  const first = updates.notes.split('\n').find((l) => l.trim() && !l.startsWith('#'))
  openMenu({
    title: `Melue ${updates.version} is ready`,
    subtitle: first?.replace(/^[-*]\s*/, '') || 'Update now? Your music and settings stay as they are.',
    items: [
      { action: 'update', label: 'Update', icon: Download },
      { action: 'later', label: 'Not now', icon: Clock },
      { action: 'never', label: 'Don’t ask again', icon: CircleSlash, apart: true },
    ],
    pick: (action) => (action === 'update' ? installUpdate() : action === 'never' ? setAskForUpdates(false) : notNow()),
  })
}

/** Downloads and installs the newer release, then starts Melue again (a phone: Android's installer takes over). */
export async function installUpdate() {
  updates.asking = false
  if (isMobile && !pretend) return installOnPhone()
  if (!found && !pretend) return
  updates.state = 'downloading'
  updates.progress = 0
  try {
    if (pretend) {
      for (let p = 1; p <= 20; p++) {
        await wait(150)
        updates.progress = p / 20
      }
      updates.state = 'installing'
      await wait(1500)
      // As the new version would, once started again.
      updates.state = 'latest'
      toast(`Updated to Melue ${updates.version}`, Sparkles, { label: "What's new", run: () => router.push({ path: '/settings', query: { whatsNew: '1' } }) })
      return
    }
    let total = 0
    let done = 0
    await found!.downloadAndInstall((event) => {
      if (event.event === 'Started') total = event.data.contentLength ?? 0
      else if (event.event === 'Progress') {
        done += event.data.chunkLength
        updates.progress = total ? done / total : null
      } else if (event.event === 'Finished') updates.state = 'installing'
    })
    const { relaunch } = await import('@tauri-apps/plugin-process')
    await relaunch()
  } catch (e) {
    updates.state = 'failed'
    updates.error = String(e)
  }
}

/** A phone: the APK downloaded (its progress shown), then Android's installer. */
async function installOnPhone() {
  if (!apk) return
  updates.state = 'downloading'
  updates.progress = downloaded ? 1 : 0
  const stop = await listen<{ done: number; total: number }>('update-download', (e) => {
    updates.progress = e.payload.total ? e.payload.done / e.payload.total : null
  })
  try {
    if (downloaded) await invoke('update_install_downloaded')
    else await invoke('update_install_apk', { url: apk.url, size: apk.size })
    downloaded = true
    // Android's installer is asking; installing replaces Melue (open it again after).
    updates.state = 'installing'
  } catch (e) {
    if (String(e).includes('allow')) {
      // Android's screen to allow it is open: back here, Update again (the file is there already).
      downloaded = true
      updates.state = 'available'
      toast('Allow Melue to install updates, then tap Update again', Download, { label: 'Update', run: installUpdate })
    } else {
      updates.state = 'failed'
      updates.error = String(e)
    }
  } finally {
    stop()
  }
}

/** "Not now": the dialog closes; About's Updates row still offers it, and the next look asks again. */
export const notNow = () => (updates.asking = false)

/** "Ask when an update is found" on or off ("Don't ask again" switches it off). */
export async function setAskForUpdates(ask: boolean) {
  updates.ask = ask
  if (!ask) updates.asking = false
  await library.setSetting(ASK_SETTING, ask ? '1' : '0').catch(() => {})
}

let started = false
/** Once, at start (desktop). */
export async function followUpdates() {
  if (started) return
  started = true
  updates.ask = (await library.setting(ASK_SETTING).catch(() => null)) !== '0'
  setTimeout(() => {
    checkForUpdate(true)
    setInterval(() => checkForUpdate(true), EVERY_MS)
  }, FIRST_MS)
}
