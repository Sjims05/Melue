<script setup lang="ts">
/**
 * Queue settings: the queue following its list (when the album or playlist playing changes, the queue changes with
 * it), with a summary of what it does for the choices; Customize opens the four choices, and "Show me how it works"
 * opens an example (TryIt.vue). Then the end of the queue: Next on the last song, and songs added to the queue when
 * it repeats.
 *
 * The queue following its list is Melue's (library/follow.rs: QueueFollow); the end of the queue is the player's
 * (playerSettings.ts).
 */
import { computed, ref } from 'vue'
import { Switch } from '@/components/ui/switch'
import { Chevron, DropValue, resetTo, Reveal, Segmented, SettingsCard, SettingsLabel, SettingsRow, SettingsTip } from '@/components/settings'
import { library, type QueueFollow } from '@/library'
import { onPageLoad } from '@/components/page/pageLoading'
import { QUEUE_FOLLOW_DEFAULTS } from './defaults'
import { loadPlayerSettings, playerSettings, resetPlayer, savePlayerSettings } from './playerSettings'
import { followSummary } from './queueFollow'
import { LIST_PLAY_DEFAULTS, listPlay } from '@/lib/listPlay'
import TryIt from './TryIt.vue'

const CHOICES = {
  removedCurrent: [['finish', 'Finish it'], ['skip', 'Skip it']],
  removedItems: [['remove', 'Remove them'], ['keep', 'Keep them']],
  newItems: [['inPlace', 'Near their place'], ['end', 'At the end'], ['next', 'Next']],
  order: [['playlist', 'The list’s'], ['queue', 'The queue’s own']],
} as const
const AT_END = [['nothing', 'Nothing'], ['first', 'First song, paused']] as const
const SONG_PLAYS = [['now', 'Now'], ['next', 'Next']] as const

const follow = ref<QueueFollow | null>(null)
const customizing = ref(false)
const trying = ref(false)
const error = ref('')

onPageLoad(async () => {
  loadPlayerSettings()
  try {
    follow.value = await library.queueFollow()
  } catch (e) {
    error.value = `Can’t read the queue settings: ${e}`
  }
})

async function save(change: Partial<QueueFollow>) {
  const before = follow.value
  if (!before) return
  // Keeping songs that leave, while skipping the playing one, makes no sense together: with Keep, it finishes.
  if (change.removedItems === 'keep') change = { ...change, removedCurrent: 'finish' }
  follow.value = { ...before, ...change }
  error.value = ''
  try {
    follow.value = await library.setQueueFollow(follow.value)
  } catch (e) {
    follow.value = before
    error.value = `Not saved: ${e}`
  }
}

const resetFollow = <K extends keyof QueueFollow>(key: K) => ({
  setting: `queue.${key}`,
  reset: resetTo(() => follow.value?.[key] ?? QUEUE_FOLLOW_DEFAULTS[key], (value) => save({ [key]: value }), QUEUE_FOLLOW_DEFAULTS[key]),
  isDefault: follow.value?.[key] === QUEUE_FOLLOW_DEFAULTS[key],
})

const summary = computed(() => (follow.value ? followSummary(follow.value) : ''))
</script>

<template>
  <SettingsLabel>Queue settings</SettingsLabel>
  <SettingsCard v-if="follow">
    <SettingsRow title="The queue follows its list" v-bind="resetFollow('enabled')">
      <template #description>
        <Transition name="swap" mode="out-in"><span :key="summary">{{ summary }}</span></Transition>
      </template>
      <Switch :model-value="follow.enabled" @update:model-value="(enabled) => save({ enabled })" />
    </SettingsRow>
    <Reveal :show="follow.enabled">
      <SettingsRow as="button" title="Customize" detail :aria-expanded="customizing" @click="customizing = !customizing">
        <template #end><Chevron :open="customizing" /></template>
      </SettingsRow>
      <Reveal :show="customizing">
        <Reveal :show="follow.removedItems === 'remove'" :line="false">
          <SettingsRow title="When the playing song leaves the list" detail v-bind="resetFollow('removedCurrent')">
            <DropValue :model-value="follow.removedCurrent" :options="CHOICES.removedCurrent" @update:model-value="(removedCurrent) => save({ removedCurrent })" />
          </SettingsRow>
        </Reveal>
        <SettingsRow title="Other songs that leave it" detail v-bind="resetFollow('removedItems')">
          <DropValue :model-value="follow.removedItems" :options="CHOICES.removedItems" @update:model-value="(removedItems) => save({ removedItems })" />
        </SettingsRow>
        <SettingsRow title="New songs go" detail v-bind="resetFollow('newItems')">
          <DropValue :model-value="follow.newItems" :options="CHOICES.newItems" @update:model-value="(newItems) => save({ newItems })" />
        </SettingsRow>
        <SettingsRow title="Order" detail v-bind="resetFollow('order')">
          <DropValue :model-value="follow.order" :options="CHOICES.order" @update:model-value="(order) => save({ order })" />
        </SettingsRow>
        <SettingsRow as="button" title="Show me how it works" detail :aria-expanded="trying" @click="trying = !trying">
          <template #end><Chevron :open="trying" /></template>
        </SettingsRow>
      </Reveal>
    </Reveal>
  </SettingsCard>
  <Reveal :show="!!follow?.enabled && customizing && trying" :line="false">
    <TryIt v-if="follow" :follow="follow" class="mt-5" />
  </Reveal>

  <SettingsCard v-if="playerSettings.value" class="mt-5">
    <SettingsRow
      title="Play starts from the first song"
      :description="
        listPlay.playFromStart
          ? 'Play on an album, playlist or folder turns shuffle off; Shuffle turns it on'
          : 'Play on an album, playlist or folder leaves shuffle as it is'
      "
      setting="queue.playFromStart"
      :reset="() => (listPlay.playFromStart = LIST_PLAY_DEFAULTS.playFromStart)"
      :is-default="listPlay.playFromStart === LIST_PLAY_DEFAULTS.playFromStart"
    >
      <Switch v-model="listPlay.playFromStart" />
    </SettingsRow>
    <SettingsRow
      title="A song from All songs plays"
      :description="
        listPlay.songPlays === 'now'
          ? 'Its play button plays it now; the playing song ends, Up next stays'
          : 'Its play button plays it after the playing song; Up next stays'
      "
      wide
      setting="queue.songPlays"
      :reset="() => (listPlay.songPlays = LIST_PLAY_DEFAULTS.songPlays)"
      :is-default="listPlay.songPlays === LIST_PLAY_DEFAULTS.songPlays"
    >
      <Segmented v-model="listPlay.songPlays" :options="SONG_PLAYS" />
    </SettingsRow>
    <SettingsRow
      title="Next on the last song"
      :description="playerSettings.value.nextAtEnd === 'first' ? 'With repeat off: back to the first song, paused' : 'With repeat off: nothing happens'"
      wide
      v-bind="resetPlayer('nextAtEnd')"
    >
      <Segmented :model-value="playerSettings.value.nextAtEnd" :options="AT_END" @update:model-value="(nextAtEnd) => savePlayerSettings({ nextAtEnd })" />
    </SettingsRow>
    <SettingsRow
      title="Keep added songs when it repeats"
      :description="
        playerSettings.value.repeatAddedTracks
          ? 'Songs added to a list’s queue play every time it starts over'
          : 'Songs added to a list’s queue play once, then leave it'
      "
      v-bind="resetPlayer('repeatAddedTracks')"
    >
      <Switch :model-value="playerSettings.value.repeatAddedTracks" @update:model-value="(repeatAddedTracks) => savePlayerSettings({ repeatAddedTracks })" />
    </SettingsRow>
  </SettingsCard>
  <Transition name="drop">
    <SettingsTip v-if="error || playerSettings.error" kind="bad" class="mt-3">{{ error || playerSettings.error }}</SettingsTip>
  </Transition>
</template>
