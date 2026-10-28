<script setup lang="ts">
/**
 * "Save battery on … songs": on a phone that can play some formats on its own audio chip (offload) only with a
 * split-second gap between songs, whether those are played there too. `choice`: what the phone can do
 * (offloadSupport.ts); the whole story (a limit of the phone, not Melue's) is its About this setting (registry.ts).
 * The Audio settings show it with the phone's real answer; the Dev page shows it for pretend phones.
 */
import { SettingsRow } from '@/components/settings'
import { Switch } from '@/components/ui/switch'
import type { OffloadChoice } from './offloadSupport'

defineProps<{ choice: OffloadChoice }>()
const on = defineModel<boolean>({ required: true })
</script>

<template>
  <SettingsRow :title="`Save battery on ${choice.withGaps} songs`" description="Your phone adds a short gap between them" v-bind="$attrs">
    <Switch v-model="on" />
  </SettingsRow>
</template>
