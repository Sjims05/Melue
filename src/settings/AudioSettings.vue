<script setup lang="ts">
/**
 * Audio settings: the volume curve and the media keys (desktop), what previous and next do there (phones: in the
 * notification and on headphones), what the queue does when Melue starts, previous restarting the song, and keeping
 * the player ready after a pause (phones), and, on a phone that can play some formats on its audio chip only with gaps
 * between songs, whether those are played there too (offloadSupport.ts). The player's settings (playerSettings.ts).
 */
import { Switch } from '@/components/ui/switch'
import { DropValue, Reveal, Segmented, SettingsCard, SettingsLabel, SettingsRow } from '@/components/settings'
import type { PlayerSettings } from '@/library'
import { isMobile } from '@/platform'
import { onPageLoad } from '@/components/page/pageLoading'
import { loadPlayerSettings, playerSettings, resetPlayer, savePlayerSettings } from './playerSettings'
import { lyricsOnline, resetLyricsOnline, setLyricsOnline } from './lyricsSetting'
import { loadOffloadSupport, offloadChoice } from './offloadSupport'
import OffloadSetting from './OffloadSetting.vue'

const CURVES = [['quadratic', 'Quadratic'], ['cubic', 'Cubic'], ['linear', 'Linear']] as const
const CURVE_LINE: Record<PlayerSettings['volumeCurve'], string> = {
  quadratic: 'Finer control at low volume (the default)',
  cubic: 'Even finer control at low volume',
  linear: 'Even steps; the quiet part goes by fast',
}
const SKIPS = [[0, 'Skip songs'], [10, 'Jump 10 s'], [30, 'Jump 30 s']] as const
const STARTS = [['paused', 'Load the last queue, paused'], ['play', 'Play it'], ['off', 'Start empty']] as const
const RESTARTS = [[0, 'Never'], [3, '3 seconds'], [5, '5 seconds'], [10, '10 seconds']] as const
const READY = [[5, '5 min'], [15, '15 min'], [30, '30 min'], [60, '1 hour']] as const

onPageLoad(loadPlayerSettings)
onPageLoad(() => lyricsOnline.load())
onPageLoad(loadOffloadSupport)
</script>

<template>
  <SettingsLabel>Audio settings</SettingsLabel>
  <SettingsCard v-if="playerSettings.value">
    <template v-if="!isMobile">
      <SettingsRow title="Volume curve" wide v-bind="resetPlayer('volumeCurve')">
        <template #description>
          <Transition name="swap" mode="out-in">
            <span :key="playerSettings.value.volumeCurve">{{ CURVE_LINE[playerSettings.value.volumeCurve] }}</span>
          </Transition>
        </template>
        <Segmented :model-value="playerSettings.value.volumeCurve" :options="CURVES" @update:model-value="(volumeCurve) => savePlayerSettings({ volumeCurve })" />
      </SettingsRow>
      <SettingsRow
        title="Media keys and the media panel"
        description="Play, pause and skip from your keyboard, headphones and the system’s media panel"
        v-bind="resetPlayer('mediaControls')"
      >
        <Switch :model-value="playerSettings.value.mediaControls" @update:model-value="(mediaControls) => savePlayerSettings({ mediaControls })" />
      </SettingsRow>
      <Reveal :show="playerSettings.value.mediaControls">
        <SettingsRow title="Previous and next do" detail wide v-bind="resetPlayer('skipSeconds')">
          <Segmented :model-value="playerSettings.value.skipSeconds" :options="SKIPS" @update:model-value="(skipSeconds) => savePlayerSettings({ skipSeconds })" />
        </SettingsRow>
      </Reveal>
    </template>
    <SettingsRow
      v-else
      title="Previous and next do"
      description="In the notification, on the lock screen, the car and headphones"
      wide
      v-bind="resetPlayer('skipSeconds')"
    >
      <Segmented :model-value="playerSettings.value.skipSeconds" :options="SKIPS" @update:model-value="(skipSeconds) => savePlayerSettings({ skipSeconds })" />
    </SettingsRow>
    <SettingsRow title="When Melue starts" description="What the queue does when you open Melue" v-bind="resetPlayer('resumeLastQueue')">
      <DropValue :model-value="playerSettings.value.resumeLastQueue" :options="STARTS" @update:model-value="(resumeLastQueue) => savePlayerSettings({ resumeLastQueue })" />
    </SettingsRow>
    <SettingsRow title="Previous restarts the song after" description="Before that, it goes to the song before" v-bind="resetPlayer('previousRestartsAfterSeconds')">
      <DropValue
        :model-value="playerSettings.value.previousRestartsAfterSeconds"
        :options="RESTARTS"
        @update:model-value="(previousRestartsAfterSeconds) => savePlayerSettings({ previousRestartsAfterSeconds })"
      />
    </SettingsRow>
    <!-- Only on a phone that can play some formats on its audio chip only with gaps between songs (its limit). -->
    <OffloadSetting
      v-if="offloadChoice"
      :choice="offloadChoice"
      :model-value="playerSettings.value.offloadWithGaps"
      v-bind="resetPlayer('offloadWithGaps')"
      @update:model-value="(offloadWithGaps) => savePlayerSettings({ offloadWithGaps })"
    />
    <SettingsRow
      title="Find lyrics online"
      description="On Wi-Fi, from LRCLIB, for songs without their own: it’s sent each song’s artist, title, album and length"
      setting="lyrics.online"
      v-bind="resetLyricsOnline()"
    >
      <Switch :model-value="lyricsOnline.on" @update:model-value="setLyricsOnline" />
    </SettingsRow>
    <template v-if="isMobile">
      <SettingsRow title="Keep the player ready after pausing" description="So the car and the notification can resume it" v-bind="resetPlayer('pausedKeepAliveMinutes')">
        <DropValue
          :model-value="playerSettings.value.pausedKeepAliveMinutes"
          :options="READY"
          @update:model-value="(pausedKeepAliveMinutes) => savePlayerSettings({ pausedKeepAliveMinutes })"
        />
      </SettingsRow>
      <SettingsRow title="Keep it ready while Android Auto is connected" v-bind="resetPlayer('keepAliveWhileCarConnected')">
        <Switch
          :model-value="playerSettings.value.keepAliveWhileCarConnected"
          @update:model-value="(keepAliveWhileCarConnected) => savePlayerSettings({ keepAliveWhileCarConnected })"
        />
      </SettingsRow>
    </template>
  </SettingsCard>
</template>
