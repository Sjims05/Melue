<script setup lang="ts">
/**
 * About: Melue's version, details to copy when something goes wrong (version, device, share and sync status, the
 * library's counts; never a password), and Melue's log (logging.rs): shared from the phone (Android's share sheet),
 * shown in its folder on a computer.
 */
import { getVersion } from '@tauri-apps/api/app'
import { invoke } from '@tauri-apps/api/core'
import { ref } from 'vue'
import { Button } from '@/components/ui/button'
import { SettingsCard, SettingsHeading, SettingsRow, SettingsTip } from '@/components/settings'
import { library } from '@/library'
import { share, sync } from '@/library/sync'
import { isAndroid, isMobile } from '@/platform'
import { onPageLoad } from '@/components/page/pageLoading'
import { ago, didWords, shareName } from './format'

const version = ref('')
const copied = ref(false)
const error = ref('')
async function showLog() {
  error.value = ''
  await invoke('log_show').catch((e) => (error.value = String(e)))
}

onPageLoad(async () => {
  version.value = await getVersion().catch(() => '')
})

/** What helps to find a problem, as text; never a password. */
async function details(): Promise<string> {
  const [info, status] = await Promise.all([share.info().catch(() => null), library.status().catch(() => null)])
  const s = sync.status
  const lines = [
    `Melue ${version.value}`,
    `Device: ${isAndroid ? 'Android' : isMobile ? 'iOS' : 'desktop'} · ${navigator.userAgent}`,
    `Share: ${info?.settings ? `${shareName(info.settings)} (${info.settings.kind}${info.settings.kind === 'smb' ? `, password ${info.settings.passwordStore}` : ''})` : 'none'}`,
    info?.needsPassword ? 'Share: waiting for its password' : '',
    s?.last ? `Last sync: ${ago(s.last.atMs)} (${s.last.trigger}) · ${didWords(s.last.result)}` : 'Last sync: none',
    s?.problem ? `Sync problem: ${JSON.stringify(s.problem)}` : '',
    status ? `Library: ${status.tracks} songs · ${status.albums} albums · ${status.folders} folders · ${status.playlists} playlists · schema ${status.schemaVersion}` : '',
  ]
  return lines.filter(Boolean).join('\n')
}

async function copy() {
  error.value = ''
  try {
    await navigator.clipboard.writeText(await details())
    copied.value = true
    setTimeout(() => (copied.value = false), 2000)
  } catch (e) {
    error.value = `Couldn’t copy: ${e}`
  }
}
</script>

<template>
  <SettingsHeading title="About" description="App information and version details" />
  <SettingsCard>
    <SettingsRow :title="`Melue ${version}`" description="Your own music, on every device" />
    <SettingsRow title="Copy details for troubleshooting" description="Version, device, share and sync status (no passwords)">
      <Button variant="outline" size="sm" @click="copy">
        <Transition name="swap" mode="out-in"><span :key="String(copied)">{{ copied ? 'Copied' : 'Copy' }}</span></Transition>
      </Button>
    </SettingsRow>
    <SettingsRow :title="isMobile ? 'Share the log' : 'Melue’s log'" description="What Melue did in the last days, as a file">
      <Button variant="outline" size="sm" @click="showLog">{{ isMobile ? 'Share' : 'Show' }}</Button>
    </SettingsRow>
  </SettingsCard>
  <SettingsTip v-if="error" kind="bad" class="mt-3">{{ error }}</SettingsTip>
</template>
