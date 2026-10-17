<script setup lang="ts">
/**
 * About: Melue's version, details to copy when something goes wrong (version, device, share and sync status, the
 * library's counts; never a password), and (phone) sharing Melue's log (logging.rs: the phone keeps one).
 *
 * Not wired yet: sharing the log (Melue can't hand a file to Android's share sheet yet).
 */
import { getVersion } from '@tauri-apps/api/app'
import { onMounted, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { SettingsBadge, SettingsCard, SettingsHeading, SettingsRow, SettingsTip } from '@/components/settings'
import { library } from '@/library'
import { share, sync } from '@/library/sync'
import { isAndroid, isMobile } from '@/platform'
import { ago, didWords, shareName } from './format'

const version = ref('')
const copied = ref(false)
const error = ref('')

onMounted(async () => {
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
    <SettingsRow v-if="isAndroid" description="What Melue did in the last days, as a file">
      <template #title>Share the log <SettingsBadge class="ml-1.5">Not wired yet</SettingsBadge></template>
      <Button variant="outline" size="sm" disabled>Share</Button>
    </SettingsRow>
  </SettingsCard>
  <SettingsTip v-if="error" kind="bad" class="mt-3">{{ error }}</SettingsTip>
</template>
