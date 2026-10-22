<script setup lang="ts">
/**
 * About: Melue's version, details to copy when something goes wrong (version, device, share and sync status, the
 * library's counts; never a password), and Melue's log (logging.rs): shared from the phone (Android's share sheet),
 * shown in its folder on a computer.
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
import { checkForUpdate, installUpdate, setAskForUpdates, updates } from '@/lib/updates'
import { Switch } from '@/components/ui/switch'

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
      return 'Installing: Melue starts again in a moment'
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

onPageLoad(async () => {
  version.value = await getVersion().catch(() => '')
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
    <SettingsRow v-if="!isMobile" title="Updates" :description="updateLine">
      <Button v-if="updates.state === 'available'" size="sm" @click="installUpdate">Update</Button>
      <Button v-else variant="outline" size="sm" :disabled="['checking', 'downloading', 'installing'].includes(updates.state)" @click="checkForUpdate()">Check now</Button>
    </SettingsRow>
    <SettingsRow
      v-if="!isMobile"
      title="Ask when an update is found"
      description="Off: no question; About shows it here"
      detail
      setting="updates.ask"
      :reset="() => setAskForUpdates(true)"
      :is-default="updates.ask"
    >
      <Switch :model-value="updates.ask" @update:model-value="setAskForUpdates" />
    </SettingsRow>
    <SettingsRow :title="isMobile ? 'Share the log' : 'Melue’s log'" description="What Melue did in the last days, as a file">
      <Button variant="outline" size="sm" @click="showLog">{{ isMobile ? 'Share' : 'Show' }}</Button>
    </SettingsRow>
  </SettingsCard>
  <SettingsTip v-if="error" kind="bad" class="mt-3">{{ error }}</SettingsTip>
  <WhatsNew v-model:open="whatsNew" :current="version" />
</template>
