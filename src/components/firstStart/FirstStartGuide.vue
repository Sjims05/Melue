<script setup lang="ts">
/**
 * The first-start guide (lib/firstStart.ts), over the whole window, a step at a time (Back, and Skip for the rest):
 * 1. Welcome (WelcomeArt.vue: what Melue is at a glance).
 * 2. Already using Melue on another device? Yes: your share (ShareSetup.vue), then this device's name: the settings,
 *    library, playlists and likes come from there. No: on.
 * 3. Make it yours: the settings that come down to taste (tastes.ts), a card per choice.
 * 4. Your music: the music folders (MusicFolders.vue; a computer starts with its Music folder), and the share for more
 *    devices (then this device's name), or later.
 * Both ways the folders and the share are on one page (a NAS as a drive can be a music folder, streamed). Then Home. The settings' own sections (the share, the folders): the guide is a calm way through them, not a copy.
 *
 * As a new device (`pretend`, Melue Dev): the share and the folders as on a fresh start, and nothing saved (the look
 * changes only for the moment). On a computer it covers the top bar: a bar to drag the window by, and its buttons.
 */
import { ArrowLeft, Check, Folder, Laptop, Plus, Smartphone } from '@lucide/vue'
import { computed, ref, shallowRef, watch } from 'vue'
import { Button } from '@/components/ui/button'
import { IconTile, SettingsCard, SettingsLabel, SettingsRow } from '@/components/settings'
import { playbackSync, sync, syncing } from '@/library/sync'
import { closeFirstStart, firstStart } from '@/lib/firstStart'
import { isDesktop, storage } from '@/platform'
import { router } from '@/router'
import MusicFolders from '@/settings/MusicFolders.vue'
import ShareSetup from '@/settings/ShareSetup.vue'
import WindowButtons from '@/shells/desktop/WindowButtons.vue'
import { CHOICE, picked } from './choice'
import TasteCards from './TasteCards.vue'
import { readTastes, type Taste } from './tastes'
import WelcomeArt from './WelcomeArt.vue'

type Step = 'welcome' | 'other' | 'share' | 'tastes' | 'music' | 'name'
const step = ref<Step>('welcome')
// Where Back goes (the share's setup: back to where it was asked).
const from = ref<Step[]>([])
// Which way it slides (on, or back).
const back = ref(false)
function go(to: Step) {
  back.value = false
  from.value.push(step.value)
  step.value = to
}
function goBack() {
  back.value = true
  step.value = from.value.pop() ?? 'welcome'
}
watch(
  () => firstStart.open,
  (open) => {
    if (!open) return
    step.value = 'welcome'
    from.value = []
  },
)

// Connected to the share (or pretending to): this device's name next. For real, the first sync starts at once (as
// Settings' Start sync): the songs already in the synced folder are matched with the share's, the rest comes down.
function connected() {
  if (!firstStart.pretend) syncing.now().catch(() => {})
  go('name')
}
watch(
  () => sync.available,
  (now, was) => (step.value === 'share' || step.value === 'music') && now && !was && connected(),
)

// ---- Make it yours: a card per choice, saved as picked (not as a new device).
// Shallow: each taste's own ref stays a ref (not unwrapped).
const tastes = shallowRef<Taste[]>([])
watch(step, async (now) => {
  if (now === 'tastes' && !tastes.value.length) tastes.value = await readTastes().catch(() => [])
  if (now === 'name') name.value = (await playbackSync.settings().catch(() => null))?.deviceName ?? ''
})
// Its cards: side by side; four in two rows; three on a phone one under another (side by side they'd be too narrow).

// ---- As a new device: the folders a fresh start has (a computer: its Music folder), and any picked, only listed here.
const pretendFolders = ref<{ name: string; where: string }[]>([])
watch(
  () => firstStart.open && firstStart.pretend,
  (on) => on && (pretendFolders.value = isDesktop ? [{ name: 'Music', where: 'Your Music folder · added by itself' }] : []),
)
async function pretendAdd() {
  const location = await storage.pickMusicFolder().catch(() => null)
  if (location) pretendFolders.value.push({ name: location.split(/[\/]/).filter(Boolean).pop() ?? location, where: location })
}

// ---- This device's name (how your other devices see it).
const name = ref('')
async function saveName() {
  if (firstStart.pretend) return
  const settings = await playbackSync.settings().catch(() => null)
  if (settings && name.value.trim() && name.value.trim() !== settings.deviceName) {
    await playbackSync.setSettings({ ...settings, deviceName: name.value.trim() }).catch(() => {})
  }
}

async function finish() {
  if (step.value === 'name') await saveName()
  await closeFirstStart()
  router.push('/').catch(() => {})
}

const DeviceIcon = isDesktop ? Laptop : Smartphone
const TITLES: Record<Step, string> = {
  welcome: 'Welcome to Melue',
  other: 'Already using Melue on another device?',
  share: 'Connect to your share',
  tastes: 'Make it yours',
  music: 'Your music',
  name: 'This device',
}
const title = computed(() => TITLES[step.value])
</script>

<template>
  <Transition name="fade">
    <div v-if="firstStart.open" class="fixed inset-0 z-50 flex flex-col overflow-y-auto bg-background" data-arrows-page>
      <!-- A computer: the window dragged by the top (nothing shows), and its buttons. -->
      <div v-if="isDesktop" class="fixed inset-x-0 top-0 z-10 flex h-10 justify-end" data-tauri-drag-region data-arrows-skip>
        <WindowButtons />
      </div>
      <div class="mx-auto flex w-full max-w-[560px] flex-col px-5 pt-[max(env(safe-area-inset-top),2.5rem)] pb-10">
        <!-- Back, and Skip (the rest). -->
        <div class="mb-8 flex h-9 items-center justify-between">
          <Button v-if="step !== 'welcome'" variant="ghost" size="sm" class="-ml-2" @click="goBack"><ArrowLeft />Back</Button>
          <span v-else />
          <Button variant="ghost" size="sm" class="-mr-2 text-muted-foreground" @click="closeFirstStart">Skip</Button>
        </div>

        <Transition :name="back ? 'guide-back' : 'guide-on'" mode="out-in">
          <div :key="step" class="flex flex-col">
            <h1 class="text-3xl font-bold tracking-tight phone:text-[28px]">{{ title }}</h1>

            <!-- 1. Welcome. -->
            <template v-if="step === 'welcome'">
              <p class="mt-3 text-[15px] text-muted-foreground">
                Your own music on your computers and your phone: playlists, likes and what you played, kept in step through a share on your network (a NAS).
              </p>
              <WelcomeArt class="mt-10" />
              <Button size="lg" class="mt-10 w-full" @click="go('other')">Get started</Button>
            </template>

            <!-- 2. Another device? -->
            <template v-else-if="step === 'other'">
              <p class="mt-3 text-[15px] text-muted-foreground">Then this one only needs your share: your settings, library, playlists and likes come from there.</p>
              <div class="mt-8 flex flex-col gap-3">
                <button type="button" :class="[CHOICE, picked(false)]" @click="go('share')">
                  <span class="block font-semibold">Yes, connect to my share</span>
                  <span class="block text-sm text-muted-foreground">Everything set up on your other device comes along</span>
                </button>
                <button type="button" :class="[CHOICE, picked(false)]" @click="go('tastes')">
                  <span class="block font-semibold">No, this is my first</span>
                  <span class="block text-sm text-muted-foreground">A few choices, then your music</span>
                </button>
              </div>
            </template>

            <!-- 3. Make it yours. -->
            <template v-else-if="step === 'tastes'">
              <p class="mt-3 text-[15px] text-muted-foreground">A few things people like either way. Any of them can be changed in Settings.</p>
              <TasteCards :tastes="tastes" :pretend="firstStart.pretend" />
              <Button size="lg" class="mt-8 w-full" @click="go('music')">Next</Button>
            </template>

            <!-- 2a / 4. The music folders and the share, on one page (Settings' own). -->
            <template v-else-if="step === 'share' || step === 'music'">
              <p class="mt-3 mb-6 text-[15px] text-muted-foreground">
                {{
                  step === 'share'
                    ? 'The folders your music is in on this device (your NAS as a drive can be one, streamed), then your share: its address, and the login your other devices use.'
                    : 'The folders your music is in. A share on your network (a NAS) keeps more devices in step: set it up now, or any time in Settings.'
                }}
              </p>
              <MusicFolders v-if="!firstStart.pretend" />
              <!-- As a new device: as the folders look on a fresh start; a folder added is only listed here. -->
              <template v-else>
                <SettingsLabel mark="This device">Music folders</SettingsLabel>
                <SettingsCard>
                  <SettingsRow v-for="folder in pretendFolders" :key="folder.where" :title="folder.name" :description="folder.where">
                    <template #lead><IconTile :icon="Folder" /></template>
                  </SettingsRow>
                  <SettingsRow as="button" title="Add folder" @click="pretendAdd">
                    <template #lead><IconTile :icon="Plus" /></template>
                  </SettingsRow>
                </SettingsCard>
              </template>
              <div class="mt-8"><ShareSetup :pretend="firstStart.pretend" @pretend-connected="connected" /></div>
              <Button size="lg" class="mt-8 w-full" @click="finish">{{ step === 'music' ? 'Done, without a share' : 'Done' }}</Button>
            </template>

            <!-- 6. This device's name (with a share: how your other devices see it). -->
            <template v-else>
              <p class="mt-3 mb-6 text-[15px] text-muted-foreground">How your other devices see this one.</p>
              <SettingsCard>
                <SettingsRow title="Name">
                  <template #lead><component :is="DeviceIcon" class="size-5 text-muted-foreground" /></template>
                  <input
                    v-model="name"
                    class="h-9 w-48 rounded-md border bg-background px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
                    :placeholder="isDesktop ? 'This computer' : 'This phone'"
                  />
                </SettingsRow>
              </SettingsCard>
              <Button size="lg" class="mt-8 w-full" @click="finish"><Check />Done</Button>
            </template>
          </div>
        </Transition>
      </div>
    </div>
  </Transition>
</template>
