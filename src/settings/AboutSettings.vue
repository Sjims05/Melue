<script setup lang="ts">
/**
 * About: Melue's version, details to copy when something goes wrong (version, device, share and sync status, the
 * library's counts; never a password), and Melue's log (logging.rs): shared from the phone (Android's share sheet),
 * shown in its folder on a computer. Report a bug (lib/report/report.ts), whether a screenshot is kept with reports,
 * and this device's reports: waiting to be sent, or sent (with their issue).
 */
import { getVersion } from '@tauri-apps/api/app'
import { invoke } from '@tauri-apps/api/core'
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { Button } from '@/components/ui/button'
import { SettingsCard, SettingsHeading, SettingsRow, SettingsTip } from '@/components/settings'
import { isMobile } from '@/platform'
import { onPageLoad } from '@/components/page/pageLoading'
import { ago } from './format'
import WhatsNew from './WhatsNew.vue'
import { troubleshootingDetails } from './troubleshooting'
import { shortcuts } from '@/lib/hotkeys'
import { checkForUpdate, installUpdate, setAskForUpdates, updates } from '@/lib/updates'
import { Switch } from '@/components/ui/switch'
import { listReports, openReport, readScreenshotSetting, reporting, setKeepScreenshots, type SavedReport } from '@/lib/report/report'
import { openUrl } from '@tauri-apps/plugin-opener'

const version = ref('')
const copied = ref(false)
const error = ref('')

// Updates (desktop): where things are, in words.
const updateLine = computed(() => {
  switch (updates.state) {
    case 'checking':
      return 'Looking for a newer version…'
    case 'latest':
      return `Up to date · looked ${ago(updates.checkedAt)}`
    case 'available':
      return `Melue ${updates.version} is ready to install`
    case 'downloading':
      return updates.progress == null ? 'Downloading…' : `Downloading · ${Math.round(updates.progress * 100)}%`
    case 'installing':
      return isMobile ? 'Android is installing it: open Melue again when it’s done' : 'Installing: Melue starts again in a moment'
    case 'failed':
      return `Couldn’t update: ${updates.error}`
    default:
      return 'From Melue’s releases on GitHub, signed'
  }
})

// What's new: from its button, or opened by a link (?whatsNew, the note after an update).
const whatsNew = ref(false)
const route = useRoute()
const router = useRouter()
watch(
  () => route.query.whatsNew,
  (asked) => {
    if (asked == null) return
    whatsNew.value = true
    router.replace({ query: { ...route.query, whatsNew: undefined } })
  },
  { immediate: true },
)

async function showLog() {
  error.value = ''
  await invoke('log_show').catch((e) => (error.value = String(e)))
}

// This device's reports (newest first), read again when the report dialog closes.
const reports = ref<SavedReport[]>([])
const showReports = ref(false)
const loadReports = async () => (reports.value = await listReports())
watch(
  () => reporting.open,
  (open) => !open && setTimeout(loadReports, 300),
)
const reportsLine = computed(() => {
  const waiting = reports.value.filter((r) => !r.sentUrl).length
  const sent = reports.value.length - waiting
  return [waiting && `${waiting} waiting to be sent`, sent && `${sent} sent`].filter(Boolean).join(' · ')
})
const reportLine = (r: SavedReport) => (r.sentUrl ? `Sent ${ago(r.sentMs ?? r.madeMs)}` : `Made ${ago(r.madeMs)} · waiting to be sent`)

onPageLoad(async () => {
  version.value = await getVersion().catch(() => '')
  await Promise.all([loadReports(), readScreenshotSetting()])
})

async function copy() {
  error.value = ''
  try {
    await navigator.clipboard.writeText(await troubleshootingDetails())
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
    <SettingsRow :title="`Melue ${version}`" description="Your own music, on every device">
      <Button variant="outline" size="sm" @click="whatsNew = true">What’s new</Button>
    </SettingsRow>
    <SettingsRow title="Copy details for troubleshooting" description="Version, device, share and sync status (no passwords)">
      <Button variant="outline" size="sm" @click="copy">
        <Transition name="swap" mode="out-in"><span :key="String(copied)">{{ copied ? 'Copied' : 'Copy' }}</span></Transition>
      </Button>
    </SettingsRow>
    <SettingsRow title="Updates" :description="updateLine">
      <Button v-if="updates.state === 'available'" size="sm" @click="installUpdate">Update</Button>
      <Button v-else variant="outline" size="sm" :disabled="['checking', 'downloading', 'installing'].includes(updates.state)" @click="checkForUpdate()">Check now</Button>
    </SettingsRow>
    <SettingsRow
      title="Ask when an update is found"
      description="Off: no question; About shows it here"
      detail
      setting="updates.ask"
      :reset="() => setAskForUpdates(true)"
      :is-default="updates.ask"
    >
      <Switch :model-value="updates.ask" @update:model-value="setAskForUpdates" />
    </SettingsRow>
    <SettingsRow title="Report a bug" :description="isMobile ? 'Tell what went wrong; you see the report before it’s sent' : 'Tell what went wrong; you see the report before it’s sent (Ctrl+Shift+B)'">
      <Button variant="outline" size="sm" @click="openReport">Report</Button>
    </SettingsRow>
    <SettingsRow
      title="Keep a screenshot with reports"
      description="The window as it was, kept with the report on your share for you; never sent"
      detail
      setting="reports.screenshot"
      :reset="() => setKeepScreenshots(false)"
      :is-default="!reporting.keepScreenshots"
    >
      <Switch :model-value="reporting.keepScreenshots" @update:model-value="setKeepScreenshots" />
    </SettingsRow>
    <SettingsRow v-if="reports.length" title="Your reports" :description="reportsLine" detail>
      <Button variant="outline" size="sm" @click="showReports = !showReports">{{ showReports ? 'Hide' : 'Show' }}</Button>
    </SettingsRow>
    <template v-if="showReports">
      <SettingsRow v-for="r in reports.slice(0, 8)" :key="r.id" :title="r.title" :description="reportLine(r)" detail>
        <Button v-if="r.sentUrl" variant="ghost" size="sm" @click="openUrl(r.sentUrl)">Open</Button>
      </SettingsRow>
    </template>
    <SettingsRow v-if="!isMobile" title="Keyboard shortcuts" description="The player and the pages from the keyboard (or press ?)">
      <Button variant="outline" size="sm" @click="shortcuts.open = true">Show</Button>
    </SettingsRow>
    <SettingsRow :title="isMobile ? 'Share the log' : 'Melue’s log'" description="What Melue did in the last days, as a file">
      <Button variant="outline" size="sm" @click="showLog">{{ isMobile ? 'Share' : 'Show' }}</Button>
    </SettingsRow>
  </SettingsCard>
  <Transition name="drop">
    <SettingsTip v-if="error" kind="bad" class="mt-3">{{ error }}</SettingsTip>
  </Transition>
  <WhatsNew v-model:open="whatsNew" :current="version" />
</template>
