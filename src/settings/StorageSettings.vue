<script setup lang="ts">
/**
 * Storage (this device's): how much space covers take, and Clean up, which removes covers no song uses any more.
 * Covers are made when songs are scanned, so they aren't cleared all at once: they wouldn't come back by themselves.
 */
import { onMounted, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { SettingsCard, SettingsLabel, SettingsRow, SettingsTip } from '@/components/settings'
import { library } from '@/library'
import { size } from './format'

const bytes = ref<number | null>(null)
const cleaned = ref<number | null>(null)
const cleaning = ref(false)
const error = ref('')

const read = async () => {
  bytes.value = await library.coversSize()
}
onMounted(() => read().catch((e) => (error.value = `Can’t read the covers’ size: ${e}`)))

async function cleanUp() {
  cleaning.value = true
  error.value = ''
  try {
    cleaned.value = await library.cleanCovers()
    await read()
  } catch (e) {
    error.value = `Couldn’t clean up: ${e}`
  } finally {
    cleaning.value = false
  }
}

// A cover is two files (a small and a large size).
const line = () => {
  if (cleaned.value === null) return `${size(bytes.value ?? 0)} · made from your songs, so they show fast`
  const covers = Math.ceil(cleaned.value / 2)
  return cleaned.value ? `${size(bytes.value ?? 0)} · ${covers} unused cover${covers === 1 ? '' : 's'} removed` : `${size(bytes.value ?? 0)} · nothing unused to remove`
}
</script>

<template>
  <SettingsLabel mark="This device">Storage</SettingsLabel>
  <SettingsCard v-if="bytes !== null">
    <SettingsRow title="Covers">
      <template #description>
        <Transition name="swap" mode="out-in"><span :key="String(cleaned)">{{ line() }}</span></Transition>
      </template>
      <Button variant="outline" size="sm" :disabled="cleaning" @click="cleanUp">{{ cleaning ? 'Cleaning up…' : 'Clean up' }}</Button>
    </SettingsRow>
  </SettingsCard>
  <SettingsTip v-if="error" kind="bad" class="mt-3">{{ error }}</SettingsTip>
</template>
