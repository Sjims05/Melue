<script setup lang="ts">
/**
 * Melue Dev only (lib/devBuild.ts; the official app has no such page): tools that make things happen now instead of
 * waiting for them (a sync, an update, an error, the sync's notification on a phone).
 */
import { getVersion } from '@tauri-apps/api/app'
import { invoke } from '@tauri-apps/api/core'
import { Sparkles } from '@lucide/vue'
import { computed, ref } from 'vue'
import { PageHeader } from '@/components/page'
import { onPageLoad } from '@/components/page/pageLoading'
import { SettingsCard, SettingsHeading, SettingsRow, SettingsTip } from '@/components/settings'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { librarySync, moving, playbackSync, syncing } from '@/library/sync'
import { isAndroid } from '@/platform'
import { openReport } from '@/lib/report/report'
import { toast } from '@/lib/toast'
import { checkForUpdate } from '@/lib/updates'
import { router } from '@/router'
import { remote, type RemoteState } from '@/library/remote'
import { ConfirmDialog, Segmented } from '@/components/settings'
import { SETTINGS } from '@/settings/registry'
import { offloadChoiceFor } from '@/settings/offloadSupport'
import OffloadSetting from '@/settings/OffloadSetting.vue'
import type { OffloadFormat } from '@/platform/player/Player'
import { applyColorPreset, COLOR_PRESETS, devColors, resetDevColors } from '@/lib/devColors'

const version = ref('')

// ---- A preview of the battery-saving playback setting on pretend phones (each phone says what it can offload; Settings
// shows the choice only where some format would have gaps). Only here: the real setting isn't changed.
const PHONES = [
  ['gapless', 'Offloads gaplessly'],
  ['some', 'Some only with gaps'],
  ['all', 'All only with gaps'],
  ['none', 'No offload'],
] as const
type Phone = (typeof PHONES)[number][0]
const PHONE_FORMATS: Record<Phone, OffloadFormat[]> = {
  gapless: [
    { format: 'MP3', offload: true, gapless: true },
    { format: 'AAC', offload: true, gapless: true },
  ],
  some: [
    { format: 'MP3', offload: true, gapless: true },
    { format: 'AAC', offload: true, gapless: false },
  ],
  all: [
    { format: 'MP3', offload: true, gapless: false },
    { format: 'AAC', offload: true, gapless: false },
  ],
  none: [],
}
const phone = ref<Phone>('some')
const previewOn = ref(true)
const previewAbout = ref(false)
const offloadInfo = SETTINGS['player.offloadWithGaps']

// ---- The light theme's page and card colours (lib/devColors.ts): the preset they match, if any.
const presetNow = computed(() => (devColors.on ? COLOR_PRESETS.find((p) => p[2] === devColors.page && p[3] === devColors.card)?.[0] : 'now'))
const pick = (which: 'page' | 'card', e: Event) => Object.assign(devColors, { [which]: (e.target as HTMLInputElement).value, on: true })
const previewChoice = computed(() => offloadChoiceFor(PHONE_FORMATS[phone.value]))
/** What the last tool did, or why it couldn't. */
const result = ref<{ ok: boolean; text: string } | null>(null)

onPageLoad(async () => {
  version.value = await getVersion().catch(() => '')
})

/** Runs a tool, saying what came of it. */
async function tool(name: string, run: () => Promise<unknown> | unknown) {
  result.value = null
  try {
    const said = await run()
    result.value = { ok: true, text: typeof said === 'string' ? said : `${name}: done` }
  } catch (e) {
    result.value = { ok: false, text: `${name}: ${e}` }
  }
}

const TOOLS: { title: string; description: string; run: () => Promise<unknown> | unknown; android?: boolean }[] = [
  { title: 'Sync now', description: 'The music and the library data, with the share', run: () => syncing.now().then((r) => `Synced: ${JSON.stringify(r)}`) },
  { title: 'Sync the library data now', description: 'Likes, plays, playlists, settings, bug reports', run: () => librarySync.now() },
  { title: 'Look at the other devices’ playback', description: 'Offers what another device played, as Across devices says', run: () => playbackSync.check() },
  {
    title: 'Look for copies now',
    description: 'Synced songs that are also elsewhere on this device',
    run: () => moving.check().then((r) => `Copies: ${r.hiddenNow} hidden now, ${r.hidden} hidden in all; near misses: ${r.nearMisses.length ? r.nearMisses.join('; ') : 'none'}`),
  },
  { title: 'The sync’s notification', description: 'A slow pretend sync, to see the "Syncing music" notification', run: () => invoke('sync_simulate_notification'), android: true },
  { title: 'A song onto the share', description: 'A short test song in “Melue test” on the share: a change there', run: () => invoke<string>('dev_share_sample', { put: true }) },
  { title: 'The song off the share', description: 'Takes it away again: a song gone from the share', run: () => invoke<string>('dev_share_sample', { put: false }) },
  { title: 'Look for an update', description: 'Pretends: a made-up next version, then the question', run: () => checkForUpdate(false) },
  {
    title: 'The note after an update',
    description: 'As a new version shows it',
    run: () => toast(`Updated to Melue ${version.value}`, Sparkles, { label: "What's new", run: () => router.push({ path: '/settings', query: { whatsNew: '1' } }) }),
  },
  {
    title: 'An error',
    description: 'An error message and an uncaught error, for a bug report’s Errors',
    run: () => {
      console.error('melue: a test error from the dev page')
      setTimeout(() => {
        throw new Error('A test error from the dev page')
      })
    },
  },
  { title: 'Report a bug', description: 'The report, as from About', run: () => openReport() },
  {
    title: 'Reach your devices',
    description: 'Each of your other devices, directly over the network, with the key on the share',
    run: async () => {
      const others = (await remote.devices()).filter((d) => !d.this)
      if (!others.length) return 'No other devices on the share yet'
      const lines = await Promise.all(
        others.map((d) =>
          remote.reach(d.id).then(
            (r) => `${d.name}: reached at ${r.address} in ${r.ms} ms (Melue ${r.hello.version}, lets others play: ${r.hello.allow})`,
            (e) => `${d.name}: ${e}`,
          ),
        ),
      )
      return lines.join(' · ')
    },
  },
  {
    title: 'Play this on another device',
    description: 'Your first online device that lets others play: what plays here goes there, and this one pauses',
    run: async () => {
      const there = (await remote.devices()).find((d) => !d.this && d.online && d.allow === 'on')
      if (!there) return 'No other device online that lets others play'
      let last: RemoteState | null = null
      let failed = ''
      const stops = await Promise.all([remote.onState((s) => (last = s)), remote.onError((e) => (failed = e))])
      try {
        const connected = await remote.connect(there.id)
        await remote.sendPlaying()
        await (await import('@/platform')).player.pause()
        await new Promise((done) => setTimeout(done, 3000))
        if (failed) return `${there.name}: ${failed}`
        const s = last as RemoteState | null
        const song = s && s.index >= 0 ? s.queue[s.index] : null
        return `${connected.name} (at ${connected.address}): ${s?.playing ? 'playing' : 'paused'} “${song?.title ?? 'nothing'}” at ${Math.round(s?.position ?? 0)} s, ${s?.queue.length ?? 0} songs in its queue`
      } finally {
        stops.forEach((stop) => stop())
      }
    },
  },
  {
    title: 'Stop playing on the other device',
    description: 'Pauses it there and closes the connection',
    run: async () => {
      await remote.command({ cmd: 'pause' }).catch(() => {})
      await remote.disconnect()
      return 'Paused there, and disconnected'
    },
  },
  { title: 'Melue’s log', description: 'Shown in its folder, or shared from a phone', run: () => invoke('log_show') },
]
</script>

<template>
  <div>
    <PageHeader title="Dev" :subtitle="`Melue Dev ${version} · tools`" />

    <SettingsHeading title="Tools" description="Things that happen by themselves, now" />
    <SettingsCard>
      <template v-for="t in TOOLS" :key="t.title">
        <SettingsRow v-if="!t.android || isAndroid" :title="t.title" :description="t.description">
          <Button variant="outline" size="sm" @click="tool(t.title, t.run)">Run</Button>
        </SettingsRow>
      </template>
    </SettingsCard>
    <SettingsHeading title="Light theme colours" description="The page and its cards, until Melue restarts (the light theme)" />
    <SettingsCard>
      <SettingsRow title="Presets" description="The ones tried; Now is the app's" wide>
        <div class="flex flex-wrap gap-1.5">
          <Button v-for="p in COLOR_PRESETS" :key="p[0]" size="sm" :variant="presetNow === p[0] ? 'default' : 'outline'" @click="applyColorPreset(p[0])">
            {{ p[1] }}
          </Button>
        </div>
      </SettingsRow>
      <SettingsRow v-for="which in ['page', 'card'] as const" :key="which" :title="which === 'page' ? 'Page' : 'Cards'" :description="devColors[which]">
        <input
          type="color"
          class="h-8 w-12 cursor-pointer rounded-md border bg-transparent"
          :value="devColors[which]"
          :aria-label="which === 'page' ? 'Page colour' : 'Card colour'"
          @input="pick(which, $event)"
        />
      </SettingsRow>
      <SettingsRow title="Borders on cards" description="A thin line around each card">
        <Switch v-model="devColors.borders" />
      </SettingsRow>
      <SettingsRow v-if="devColors.borders" title="Lines between settings" description="Border colour: as thin as the border (1 px, not 2)" detail wide>
        <Segmented
          v-model="devColors.lines"
          :options="[
            ['page', 'Page colour'],
            ['border', 'Border colour'],
          ]"
        />
      </SettingsRow>
      <SettingsRow title="Back to the app's" description="Removes the colours and borders picked here">
        <Button variant="outline" size="sm" :disabled="!devColors.on && !devColors.borders" @click="resetDevColors">Reset</Button>
      </SettingsRow>
    </SettingsCard>
    <SettingsHeading title="Battery-saving playback" description="The setting in Settings, Audio, as it looks on other phones" />
    <Segmented v-model="phone" :options="PHONES" class="mb-3" />
    <template v-if="previewChoice">
      <SettingsCard>
        <OffloadSetting v-model="previewOn" :choice="previewChoice" />
      </SettingsCard>
      <button type="button" class="mt-2 px-1 text-[13.5px] text-muted-foreground underline-offset-2 hover:underline" @click="previewAbout = true">
        About this setting (a long press in Settings)
      </button>
    </template>
    <p v-else class="px-1 text-[13.5px] text-muted-foreground">Nothing shows in Settings on such a phone: there's nothing to choose.</p>
    <ConfirmDialog v-model:open="previewAbout" :title="offloadInfo.title" :text="offloadInfo.help" />
    <Transition name="drop">
      <SettingsTip v-if="result" :kind="result.ok ? 'ok' : 'bad'" class="mt-3">{{ result.text }}</SettingsTip>
    </Transition>
  </div>
</template>
