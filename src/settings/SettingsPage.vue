<script setup lang="ts">
/**
 * The settings page, as in the design (Frame 2): General, Syncing (with Mobile data), Playback, Library, Display and
 * Storage, About. Each part is its own component; this puts them in order. "Set up" (the status card, the triggers'
 * tip) goes to the share's form under Library. It gives the rows' right-click menus their extras (settingsMenu.ts),
 * and opens at a setting when asked (?setting=sync.atStart).
 */
import { nextTick, onMounted, provide, ref } from 'vue'
import { SETTING_MENU, SettingsHeading, SettingsLabel } from '@/components/settings'
import { isAndroid } from '@/platform'
import AboutSettings from './AboutSettings.vue'
import AcrossDevices from './AcrossDevices.vue'
import AndroidSource from './AndroidSource.vue'
import AudioSettings from './AudioSettings.vue'
import DisplaySettings from './DisplaySettings.vue'
import DropSongs from './DropSongs.vue'
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
import { createSettingsMenu, goToSetting } from './settingsMenu'

provide(SETTING_MENU, createSettingsMenu())

// A setting's link: the page opens at it (once its rows are there).
onMounted(() => {
  const asked = new URLSearchParams(location.search).get('setting')
  if (asked) setTimeout(() => nextTick(() => goToSetting(asked)), 600)
})

const shareSection = ref<HTMLElement>()
const toShare = () => shareSection.value?.scrollIntoView({ behavior: 'smooth', block: 'start' })
</script>

<template>
  <main class="mx-auto max-w-[960px] px-4 pt-6 pb-16">
    <h1 class="mb-6 text-[28px] font-bold tracking-tight">Settings</h1>

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
    <DropSongs />
  </main>
</template>
