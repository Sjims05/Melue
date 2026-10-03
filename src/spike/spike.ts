/**
 * Shared parts of the spike test pages (mobile and desktop): the log, SMB, song tags,
 * the storage picker, the wake lock and SQLite. Temporary, until the real UI replaces the spike pages.
 *
 * SMB credentials are read from .env (gitignored), see .env.example.
 */
import { appDataDir, join } from '@tauri-apps/api/path'
import { readFile } from '@tauri-apps/plugin-fs'
import { parseBuffer } from 'music-metadata'
import { ref } from 'vue'
import { storage } from '@/platform'
import { SmbSource } from '@/sources/SmbSource'

// ---- Logging -------------------------------------------------------
export const log = ref<string[]>([])
export function logMsg(msg: string) {
  const line = `[${new Date().toLocaleTimeString()}] ${msg}`
  console.log(line)
  log.value = [line, ...log.value].slice(0, 100)
}

// ---- SMB settings come from .env (see .env.example) -----------------
const SMB_HOST = import.meta.env.VITE_SMB_HOST
const SMB_USER = import.meta.env.VITE_SMB_USER
const SMB_PASS = import.meta.env.VITE_SMB_PASS
const SMB_SHARE = import.meta.env.VITE_SMB_SHARE
export const SMB_MUSIC_PATH = import.meta.env.VITE_SMB_MUSIC_PATH
// -----------------------------------------------------------------------

export const source = new SmbSource()
let connected = false

export async function ensureConnected() {
  if (connected) return
  await source.connect({ host: SMB_HOST, user: SMB_USER, pass: SMB_PASS, share: SMB_SHARE })
  connected = true
  logMsg('SMB: connected')
}

// ---- Song names from the files' own tags ---------------------------
export type TrackInfo = { title: string; artist?: string; album?: string }
const trackInfoCache = new Map<string, TrackInfo>()

// Title, artist(s) and album from the file's tags; `fallbackTitle` when it has none.
// Only successful reads are remembered: a file that isn't downloaded yet is read again next time.
export async function readTrackInfo(path: string, fallbackTitle: string): Promise<TrackInfo> {
  const cached = trackInfoCache.get(path)
  if (cached) return cached
  try {
    const { common } = await parseBuffer(await readFile(path), { mimeType: 'audio/mpeg' }, { skipCovers: true })
    const artists = common.artists?.length ? common.artists.join(', ') : common.artist
    const info = { title: common.title || fallbackTitle, artist: artists || undefined, album: common.album || undefined }
    trackInfoCache.set(path, info)
    return info
  } catch (e) {
    logMsg(`Tags: couldn't read ${path}: ${e}`)
    return { title: fallbackTitle }
  }
}

/** The four songs section 4 downloads (they stay in appData between launches), as queue items. */
export async function testTracks() {
  const paths = await Promise.all([0, 1, 2, 3].map(async (i) => join(await appDataDir(), `spike-queue-${i}.mp3`)))
  return Promise.all(paths.map(async (src, i) => ({ src, id: i + 1, ...(await readTrackInfo(src, `Track ${i + 1}`)) })))
}

// =====================================================================
// SECTION 1: SMB — connect, list, download one file
// =====================================================================
/** Where section 1 saved its download (played by the mobile page's section 3). */
export const localCopyPath = ref<string | null>(null)

export async function smbListFiles() {
  try {
    await ensureConnected()
    const entries = await source.listFiles(SMB_MUSIC_PATH)
    logMsg(`SMB: found ${entries.length} entries`)
    for (const e of entries.slice(0, 20)) {
      logMsg(`  - [${e.isDirectory ? 'DIR' : 'FILE'}] ${e.name} (${e.size} bytes)`)
    }
  } catch (e) {
    logMsg(`SMB list FAILED: ${e}`)
  }
}

export async function smbDownloadOneFile() {
  try {
    await ensureConnected()
    const entries = await source.listFiles(SMB_MUSIC_PATH)
    const mp3 = entries.find((e) => !e.isDirectory && e.name.toLowerCase().endsWith('.mp3'))
    if (!mp3) {
      logMsg('SMB: no mp3 found')
      return
    }

    const destPath = await join(await appDataDir(), 'spike-test.mp3')

    logMsg(`SMB: downloading ${mp3.name}...`)
    await source.copyFile(`${SMB_MUSIC_PATH}/${mp3.name}`, destPath)

    localCopyPath.value = destPath
    logMsg(`SMB: downloaded to ${destPath}`)
  } catch (e) {
    logMsg(`SMB download FAILED: ${e}`)
  }
}

// =====================================================================
// SECTION 2: (future feature demo) pick where the music is stored.
// Android: the system picker (also SD cards). Desktop: a folder dialog.
// Independent of the SMB flow above.
// =====================================================================
export async function pickLocalStorageFolder() {
  try {
    const folder = await storage.pickMusicFolder()
    if (!folder) {
      logMsg('Storage picker: cancelled')
      return
    }
    logMsg(`Storage picker: picked ${folder} (not wired to anything yet — just proving the picker works)`)
  } catch (e) {
    logMsg(`Storage picker FAILED: ${e}`)
  }
}

// =====================================================================
// SECTION 5: Screen Wake Lock — request / release
// =====================================================================
let wakeLockSentinel: any = null

export async function requestWakeLock() {
  try {
    if (!('wakeLock' in navigator)) {
      logMsg('Wake lock: navigator.wakeLock not available in this WebView')
      return
    }
    wakeLockSentinel = await (navigator as any).wakeLock.request('screen')
    logMsg('Wake lock acquired — leave the device alone and see if the screen stays on')
    wakeLockSentinel.addEventListener('release', () => {
      logMsg('Wake lock was released (by OS or by us)')
    })
  } catch (e) {
    logMsg(`Wake lock FAILED: ${e}`)
  }
}

export async function releaseWakeLock() {
  try {
    if (wakeLockSentinel) {
      await wakeLockSentinel.release()
      wakeLockSentinel = null
      logMsg('Wake lock released manually')
    } else {
      logMsg('No active wake lock to release')
    }
  } catch (e) {
    logMsg(`Wake lock release FAILED: ${e}`)
  }
}

document.addEventListener('visibilitychange', async () => {
  if (wakeLockSentinel !== null && document.visibilityState === 'visible') {
    await requestWakeLock()
  }
})

// =====================================================================
// SECTION 6: SQLite write/read
// =====================================================================
export async function sqliteTest() {
  try {
    const Database = (await import('@tauri-apps/plugin-sql')).default
    const db = await Database.load('sqlite:spike-test.db')
    await db.execute('CREATE TABLE IF NOT EXISTS test (id INTEGER PRIMARY KEY, name TEXT)')
    await db.execute('INSERT INTO test (name) VALUES ($1)', ['hello from spike'])
    const rows = await db.select<{ id: number; name: string }[]>('SELECT * FROM test')
    logMsg(`SQLite: rows = ${JSON.stringify(rows)}`)
  } catch (e) {
    logMsg(`SQLite FAILED: ${e}`)
  }
}
