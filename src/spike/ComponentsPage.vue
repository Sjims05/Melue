<script setup lang="ts">
/**
 * The spike's Components page: Phase 3's finished components, as they will be in the app, each from Simon's design
 * (and the playground's component lab). "Not wired yet" marks a component whose setting Melue doesn't have yet.
 */
import { ChevronRight, Copy, Folder, HardDrive, Cloud } from '@lucide/vue'
import { reactive } from 'vue'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import {
  Chips,
  DropValue,
  Hint,
  IconTile,
  RadioMark,
  Segmented,
  SettingsBadge,
  SettingsCard,
  SettingsHeading,
  SettingsLabel,
  SettingsRow,
  SettingsTip,
  StatusDot,
  Stepper,
} from '@/components/settings'
import { themeMode } from '@/lib/theme'
import AcrossDevices from '@/settings/AcrossDevices.vue'
import SyncStatusCard from '@/settings/SyncStatusCard.vue'

// The building blocks' demo values (nothing here is a real setting).
const demo = reactive({
  on: true,
  chips: 15,
  segment: 'quadratic' as 'quadratic' | 'cubic' | 'linear',
  limit: 250 as number | 'off',
  steps: 5,
  device: 'system',
})
const DEVICES = [
  ['system', 'System default', 'Follows Windows’ choice · now Speakers (Realtek)'],
  ['sonar', 'SteelSeries Sonar - Media', ''],
  ['speakers', 'Speakers (Realtek)', ''],
] as const
</script>

<template>
  <main class="mx-auto max-w-[960px] px-4 pt-6 pb-16">
    <header class="mb-8 flex flex-wrap items-end justify-between gap-4">
      <div>
        <h1 class="text-[28px] font-bold tracking-tight">Components</h1>
        <p class="mt-1 text-sm text-muted-foreground">Phase 3’s finished components, as they will be in the app.</p>
      </div>
      <Segmented v-model="themeMode" :options="[['system', 'System'], ['dark', 'Dark'], ['light', 'Light']]" />
    </header>

    <SettingsHeading title="General" description="Settings: the sync status and the other devices. Wired to Melue." />
    <SettingsLabel>General settings</SettingsLabel>
    <SyncStatusCard />
    <AcrossDevices />

    <div class="h-14" />
    <SettingsHeading title="Building blocks" description="What every settings component is made of (Simon’s design)." />

    <SettingsLabel>Rows and controls</SettingsLabel>
    <SettingsCard>
      <SettingsRow title="A switch" description="Gets new music as soon as you open it">
        <Switch v-model="demo.on" />
      </SettingsRow>
      <SettingsRow title="Chips" description="One of a few values" wide>
        <Chips v-model="demo.chips" :options="[[0, 'Never'], [5, '5 min'], [15, '15 min'], [60, '1 hour']]" />
      </SettingsRow>
      <SettingsRow title="Segments" description="One of a few choices" wide>
        <Segmented v-model="demo.segment" :options="[['quadratic', 'Quadratic'], ['cubic', 'Cubic'], ['linear', 'Linear']]" />
      </SettingsRow>
      <SettingsRow title="A detail row with a dropdown value" description="Per month; then songs wait for Wi-Fi" detail>
        <DropValue v-model="demo.limit" :options="[['off', 'No limit'], [250, '250 MB'], [500, '500 MB'], [1000, '1 GB']]" />
      </SettingsRow>
      <SettingsRow title="A detail row with a stepper" detail>
        <Stepper v-model="demo.steps" :steps="[1, 2, 3, 5, 10, 20, 50]" />
      </SettingsRow>
      <SettingsRow as="button" title="A row that opens something" description="TROLLGANG">
        <template #end><ChevronRight class="size-[18px] text-foreground" /></template>
      </SettingsRow>
    </SettingsCard>

    <SettingsLabel mark="This device">A list: its icons share one column</SettingsLabel>
    <SettingsCard>
      <SettingsRow v-for="[id, name, sub] in DEVICES" :key="id" as="button" :title="name" :description="sub || undefined" @click="demo.device = id">
        <template #end><RadioMark :on="demo.device === id" /></template>
      </SettingsRow>
    </SettingsCard>

    <SettingsLabel>Tiles, dots, badges, a hint</SettingsLabel>
    <SettingsCard>
      <SettingsRow title="Music" description="1.012 songs · 2 only here · 105 already synced">
        <template #lead><IconTile :icon="Folder" /></template>
      </SettingsRow>
      <SettingsRow description="218 songs · your share, copied here">
        <template #lead><IconTile :icon="Cloud" /></template>
        <template #title>Synced music <SettingsBadge class="ml-1.5">Synced</SettingsBadge></template>
      </SettingsRow>
      <SettingsRow title="Y:\Family music" description="486 songs · streamed from \\nas\family">
        <template #lead><IconTile :icon="HardDrive" /></template>
      </SettingsRow>
      <SettingsRow title="Video Killed The Radio Star" description="The Buggles · the synced one plays" detail>
        <template #lead><span class="grid w-[34px] place-items-center"><Hint :icon="Copy" text="A copy: this song is also in your synced music, and Melue plays that one" /></span></template>
        <span class="text-[13px] text-muted-foreground">9,8 MB</span>
      </SettingsRow>
      <SettingsRow title="simon@nas/music · Synced" description="Synced 12 min ago · 3 copied, 1 moved">
        <template #lead><StatusDot kind="ok" /></template>
        <Button variant="outline" size="sm">Sync now</Button>
      </SettingsRow>
    </SettingsCard>

    <SettingsLabel>Tips and buttons</SettingsLabel>
    <div class="grid gap-3">
      <SettingsTip>
        Tip: Look under Library to set up a share and folders first.
        <template #action><button type="button" class="text-[13px] font-medium underline underline-offset-3">Set one up</button></template>
      </SettingsTip>
      <SettingsTip kind="ok">Connected: 1,240 songs in nas/music</SettingsTip>
      <SettingsTip kind="bad">Couldn’t reach the share</SettingsTip>
      <div class="flex flex-wrap gap-2">
        <Button size="sm">Start sync</Button>
        <Button variant="outline" size="sm">Change folder</Button>
        <Button variant="danger" size="sm">Remove share</Button>
      </div>
    </div>
  </main>
</template>
