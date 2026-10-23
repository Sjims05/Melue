<script setup lang="ts">
/**
 * Melue Dev only (lib/devBuild.ts; the official app has no such page): the test pass before a release
 * (dev/testPass.ts), and tools that make things happen now instead of waiting for them (a sync, an update, an error,
 * the sync's notification on a phone).
 */
import { getVersion } from '@tauri-apps/api/app'
import { invoke } from '@tauri-apps/api/core'
import { Sparkles } from '@lucide/vue'
import { computed, ref } from 'vue'
import { PageHeader } from '@/components/page'
import { onPageLoad } from '@/components/page/pageLoading'
import { Segmented, SettingsCard, SettingsHeading, SettingsRow, SettingsTip } from '@/components/settings'
import { Button } from '@/components/ui/button'
import { TEST_PASS, loadTestPass, resetTestPass, setTest, testPass, type TestState } from '@/dev/testPass'
import { librarySync, moving, playbackSync, syncing } from '@/library/sync'
import { isAndroid } from '@/platform'
import { openReport } from '@/lib/report/report'
import { toast } from '@/lib/toast'
import { checkForUpdate } from '@/lib/updates'
import { router } from '@/router'

const version = ref('')
/** What the last tool did, or why it couldn't. */
const result = ref<{ ok: boolean; text: string } | null>(null)

onPageLoad(async () => {
  version.value = await getVersion().catch(() => '')
  await loadTestPass()
})

const STATES = [
  ['untested', '–'],
  ['passed', 'Passed'],
  ['failed', 'Failed'],
  ['you', 'You'],
] as const satisfies readonly (readonly [TestState, string])[]

const summary = computed(() => {
  const all = Object.values(testPass)
  const count = (state: TestState) => all.filter((t) => t.state === state).length
  return `${count('passed')} passed · ${count('failed')} failed · ${count('untested')} to test · ${count('you')} yours`
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
  { title: 'Melue’s log', description: 'Shown in its folder, or shared from a phone', run: () => invoke('log_show') },
]
</script>

<template>
  <div>
    <PageHeader title="Dev" :subtitle="`Melue Dev ${version} · tests and tools`" />

    <SettingsHeading title="Test pass" :description="summary" />
    <template v-for="group in TEST_PASS" :key="group.group">
      <h3 class="mt-4 mb-2 text-xs font-semibold tracking-wide text-muted-foreground uppercase">{{ group.group }}</h3>
      <SettingsCard>
        <SettingsRow v-for="item in group.items" :key="item.id" :title="item.what" :description="testPass[item.id]?.note || item.how" wide>
          <Segmented
            v-if="testPass[item.id]"
            :model-value="testPass[item.id].state"
            :options="STATES"
            @update:model-value="(state: TestState) => setTest(item.id, state)"
          />
        </SettingsRow>
      </SettingsCard>
    </template>
    <div class="mt-3 flex justify-end">
      <Button variant="ghost" size="sm" class="text-muted-foreground" @click="resetTestPass">Start the pass again</Button>
    </div>

    <SettingsHeading title="Tools" description="Things that happen by themselves, now" />
    <SettingsCard>
      <template v-for="t in TOOLS" :key="t.title">
        <SettingsRow v-if="!t.android || isAndroid" :title="t.title" :description="t.description">
          <Button variant="outline" size="sm" @click="tool(t.title, t.run)">Run</Button>
        </SettingsRow>
      </template>
    </SettingsCard>
    <Transition name="drop">
      <SettingsTip v-if="result" :kind="result.ok ? 'ok' : 'bad'" class="mt-3">{{ result.text }}</SettingsTip>
    </Transition>
  </div>
</template>
