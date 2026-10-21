<script setup lang="ts">
/**
 * Sync & settings (the settings page, as in the design, Frame 2): General, Syncing (with Mobile data), Playback,
 * Library, Display and Storage, About; under the title, the last sync in short. Each part is its own component; this puts them in order. "Set up" (the status card, the triggers'
 * tip) goes to the share's form under Library. It gives the rows' right-click menus their extras (settingsMenu.ts),
 * and opens at a setting when asked (?setting=sync.atStart).
 */
import { computed, nextTick, onMounted, provide, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { PageHeader } from '@/components/page'
import { SETTING_MENU, SettingsHeading, SettingsLabel } from '@/components/settings'
import { isAndroid } from '@/platform'
import AboutSettings from './AboutSettings.vue'
import AcrossDevices from './AcrossDevices.vue'
import AndroidSource from './AndroidSource.vue'
import AudioSettings from './AudioSettings.vue'
import DisplaySettings from './DisplaySettings.vue'
import LibraryStatus from './LibraryStatus.vue'
import MobileData from './MobileData.vue'
import MusicFolders from './MusicFolders.vue'
import OutputDevice from './OutputDevice.vue'
import PlaylistSettings from './PlaylistSettings.vue'
import QueueSettings from './QueueSettings.vue'
import ShareSetup from './ShareSetup.vue'
import SongsAndLayout from './SongsAndLayout.vue'
import StorageSettings from './StorageSettings.vue'
import SyncStatusCard from './SyncStatusCard.vue'
import SyncTriggers from './SyncTriggers.vue'
import { sync } from '@/library/sync'
import { ago } from './format'
import { providePageLoading } from '@/components/page/pageLoading'
import { createSettingsMenu, goToSetting } from './settingsMenu'
import { useShownSync } from './shownSync'
import { settingsTouched } from './sharedSettings'

provide(SETTING_MENU, createSettingsMenu())

// Shown once its parts have their first data (pageLoading.ts); a skeleton until then: the first sections' shapes.
const ready = providePageLoading()
const SKELETON = [{ rows: 1 }, { rows: 3 }, { rows: 4 }]

// The grey line under the title: the last sync in short ("Synced just now · 2 copied · 0 moved · 0 removed"), or the
// one running (held a moment, like the status card), and "12 min ago" moving on by itself (format.ts).
const shown = useShownSync()
const summary = computed(() => {
  const s = sync.status
  if (!sync.available) return 'No share set up'
  if (shown.value) return shown.value.filesTotal ? `Syncing ${shown.value.filesDone} of ${shown.value.filesTotal}` : 'Syncing…'
  if (s?.problem) return 'Last sync didn’t finish: see below'
  if (!s?.last) return 'Not synced yet'
  const r = s.last.result
  return `Synced ${ago(s.last.atMs)} · ${r.copied} copied · ${r.moved} moved · ${r.removed} removed`
})

// A setting's link (melue://settings/<id>, pasted in Search: ?setting=<id>): the page opens at it, once its rows are
// there (and it has slid in).
const route = useRoute()
const router = useRouter()
function openAt(asked: unknown) {
  if (typeof asked !== 'string' || !asked) return
  setTimeout(() => nextTick(() => goToSetting(asked)), 600)
  // Done: opening Settings again (the dots) doesn't go to it again.
  if (route.query.setting) router.replace({ query: {} })
}
watch(() => route.query.setting, openAt, { immediate: true })
onMounted(() => openAt(new URLSearchParams(location.search).get('setting')))

const shareSection = ref<HTMLElement>()
const toShare = () => shareSection.value?.scrollIntoView({ behavior: 'smooth', block: 'start' })
</script>

<template>
  <!-- 960 px: wider, the settings' rows get hard to read across (Melue's design). -->
  <!-- Used (a click, a change): the settings shared across devices are checked a moment later (sharedSettings.ts). -->
  <div class="max-w-[960px] pb-16" @click.capture="settingsTouched" @change.capture="settingsTouched">
    <PageHeader title="Sync & settings" :subtitle="summary" />

    <!-- Until its parts have their first data: a skeleton, then the page all at once (not section by section). -->
    <div class="relative">
      <Transition name="fade">
        <div v-if="!ready" class="pointer-events-none absolute inset-x-0 top-0 z-10 bg-background" aria-hidden="true">
          <template v-for="section in SKELETON" :key="section.rows">
            <div class="mt-5 mb-2 h-4 w-32 rounded bg-card first:mt-0" />
            <div class="rounded-xl bg-card">
              <div v-for="row in section.rows" :key="row" class="settings-row flex animate-pulse items-center gap-4 border-background [&+&]:border-t-2">
                <div class="flex flex-1 flex-col gap-2">
                  <div class="h-3.5 w-1/3 rounded bg-well" />
                  <div class="h-3 w-1/2 rounded bg-well" />
                </div>
                <div class="h-7 w-20 rounded-md bg-well" />
              </div>
            </div>
          </template>
        </div>
      </Transition>
      <div class="transition-opacity duration-300" :class="ready ? 'opacity-100' : 'opacity-0'">
        <SettingsLabel>General settings</SettingsLabel>
        <SyncStatusCard @set-up="toShare" />
        <AcrossDevices />

        <SettingsHeading title="Syncing" />
        <SyncTriggers @set-up="toShare" />
        <SongsAndLayout />
        <MobileData />

        <SettingsHeading title="Playback" />
        <QueueSettings />
        <AudioSettings />
        <OutputDevice />

        <SettingsHeading title="Library" description="Your music folders, and the share" />
        <AndroidSource v-if="isAndroid" />
        <LibraryStatus />
        <MusicFolders />
        <div ref="shareSection" class="scroll-mt-16" />
        <ShareSetup />
        <PlaylistSettings />

        <DisplaySettings />
        <StorageSettings />

        <AboutSettings />
      </div>
    </div>
  </div>
</template>
