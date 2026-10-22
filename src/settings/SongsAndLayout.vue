<script setup lang="ts">
/**
 * What syncing does with songs (removing those gone from the share, asking first above a number; moving new songs to
 * the share), and how the share's folders are laid out. Changing the layout offers to put the share's songs into it
 * (they move on the share, for every device), with an example of what moves.
 */
import { computed, ref, watch } from 'vue'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { resetTo, Reveal, Segmented, SettingsCard, SettingsLabel, SettingsRow, SettingsTip, Stepper } from '@/components/settings'
import { describeSyncFailure, shareLayout, sync, type LayoutStatus, type ReorganiseResult, type ShareLayout } from '@/library/sync'
import { onPageLoad } from '@/components/page/pageLoading'
import { SYNC_DEFAULTS } from './defaults'
import { loadSyncSettings, resetSync, saveSyncSettings, syncSettings } from './syncSettings'

const LAYOUTS = [['artistAlbum', 'Artist / Album'], ['oneFolder', 'One folder']] as const
const EXAMPLE: Record<ShareLayout, string> = {
  artistAlbum: 'ABBA / Waterloo / Waterloo.mp3 · changes it on every device',
  oneFolder: 'Waterloo.mp3 · changes it on every device',
}

const layout = ref<LayoutStatus | null>(null)
const reorganising = ref(false)
const reorganised = ref<ReorganiseResult | null>(null)
const error = ref('')

const loadLayout = async () => {
  layout.value = sync.available ? await shareLayout.status().catch(() => null) : null
}
onPageLoad(async () => {
  await loadSyncSettings()
  await loadLayout()
})
watch(() => sync.available, loadLayout)

async function pickLayout(shareLayout: ShareLayout) {
  reorganised.value = null
  error.value = ''
  await saveSyncSettings({ shareLayout })
  await loadLayout()
}

const toMove = computed(() => (layout.value && !layout.value.applied ? layout.value.toMove : 0))
const example = computed(() => layout.value?.examples[0])

async function reorganise() {
  reorganising.value = true
  error.value = ''
  try {
    reorganised.value = await shareLayout.reorganise()
    await loadLayout()
  } catch (e) {
    error.value = describeSyncFailure(e)
  } finally {
    reorganising.value = false
  }
}

const didWords = (r: ReorganiseResult) =>
  [`${r.moved} moved`, r.playlistsUpdated && `${r.playlistsUpdated} playlists updated`, r.failed.length && `${r.failed.length} failed`]
    .filter(Boolean)
    .join(' · ')
</script>

<template>
  <template v-if="syncSettings.value">
    <SettingsLabel>Songs</SettingsLabel>
    <SettingsCard>
      <SettingsRow title="Remove songs gone from the share" description="Songs deleted from the share are removed here too" v-bind="resetSync('removeGone')">
        <Switch :model-value="syncSettings.value.removeGone" @update:model-value="(removeGone) => saveSyncSettings({ removeGone })" />
      </SettingsRow>
      <Reveal :show="syncSettings.value.removeGone">
        <SettingsRow title="Ask first when it’s more than" description="A whole album gone at once is usually a mistake" detail v-bind="resetSync('askBeforeRemoving')">
          <Stepper
            :model-value="syncSettings.value.askBeforeRemoving"
            :steps="[1, 2, 3, 5, 10, 20, 50]"
            @update:model-value="(askBeforeRemoving) => saveSyncSettings({ askBeforeRemoving })"
          />
        </SettingsRow>
      </Reveal>
      <SettingsRow title="Move new songs to sync automatically" description="Songs you add to your music folders go to the share" v-bind="resetSync('moveNewSongs')">
        <Switch :model-value="syncSettings.value.moveNewSongs" @update:model-value="(moveNewSongs) => saveSyncSettings({ moveNewSongs })" />
      </SettingsRow>
    </SettingsCard>

    <SettingsLabel>On the share</SettingsLabel>
    <SettingsCard>
      <SettingsRow
        title="Folders on the share"
        wide
        setting="sync.shareLayout"
        :reset="resetTo(() => syncSettings.value?.shareLayout ?? SYNC_DEFAULTS.shareLayout, pickLayout, SYNC_DEFAULTS.shareLayout)"
        :is-default="syncSettings.value.shareLayout === SYNC_DEFAULTS.shareLayout"
      >
        <template #description>
          <Transition name="swap" mode="out-in">
            <span :key="syncSettings.value.shareLayout">{{ EXAMPLE[syncSettings.value.shareLayout] }}</span>
          </Transition>
        </template>
        <Segmented :model-value="syncSettings.value.shareLayout" :options="LAYOUTS" @update:model-value="pickLayout" />
      </SettingsRow>
      <Reveal :show="toMove > 0 || !!reorganised">
        <SettingsRow v-if="reorganised && !toMove" title="The share is in this layout" :description="didWords(reorganised)" detail />
        <SettingsRow
          v-else
          :title="`${toMove} song${toMove === 1 ? '' : 's'} on the share aren’t in this layout`"
          :description="example ? `For example: ${example.from} → ${example.to}` : undefined"
          detail
        >
          <Button size="sm" :disabled="reorganising" @click="reorganise">{{ reorganising ? 'Moving…' : 'Reorganise' }}</Button>
        </SettingsRow>
      </Reveal>
    </SettingsCard>
  </template>
  <Transition name="drop">
    <SettingsTip v-if="syncSettings.error || error" kind="bad" class="mt-3">{{ error || syncSettings.error }}</SettingsTip>
  </Transition>
</template>
