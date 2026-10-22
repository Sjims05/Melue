/**
 * What helps to find a problem, as text (About's "Copy details for troubleshooting", and an error's "Copy with
 * troubleshooting details"): Melue's version, the device, the share and the sync, the library's counts. Never a password.
 */
import { getVersion } from '@tauri-apps/api/app'
import { library } from '@/library'
import { share, sync } from '@/library/sync'
import { isAndroid, isMobile } from '@/platform'
import { ago, didWords, shareName } from './format'

export async function troubleshootingDetails(): Promise<string> {
  const [version, info, status] = await Promise.all([
    getVersion().catch(() => '?'),
    share.info().catch(() => null),
    library.status().catch(() => null),
  ])
  const s = sync.status
  const lines = [
    `Melue ${version}`,
    `Device: ${isAndroid ? 'Android' : isMobile ? 'iOS' : 'desktop'} · ${navigator.userAgent}`,
    `Share: ${info?.settings ? `${shareName(info.settings)} (${info.settings.kind}${info.settings.kind === 'smb' ? `, password ${info.settings.passwordStore}` : ''})` : 'none'}`,
    info?.needsPassword ? 'Share: waiting for its password' : '',
    s?.last ? `Last sync: ${ago(s.last.atMs)} (${s.last.trigger}) · ${didWords(s.last.result)}` : 'Last sync: none',
    s?.problem ? `Sync problem: ${JSON.stringify(s.problem)}` : '',
    status ? `Library: ${status.tracks} songs · ${status.albums} albums · ${status.folders} folders · ${status.playlists} playlists · schema ${status.schemaVersion}` : '',
  ]
  return lines.filter(Boolean).join('\n')
}
