<script setup lang="ts">
/**
 * Android: where the music comes from: all music on the phone (it needs the music permission) or only folders you
 * choose; covers from images next to songs (the photos permission); and showing the synced music in other apps.
 *
 * "All music" is Melue's android_media folder: switching to chosen folders removes it (asked first).
 */
import { Check } from '@lucide/vue'
import { computed, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { ConfirmDialog, Reveal, Segmented, SettingsCard, SettingsLabel, SettingsRow, SettingsTip } from '@/components/settings'
import { library, type FolderCovers, type MusicAccess } from '@/library'
import { share, sync } from '@/library/sync'
import { storage } from '@/platform'
import { onPageLoad } from '@/components/page/pageLoading'
import { libraryState, refreshLibrary, startLibraryState } from './libraryState'

const SOURCES = [['all', 'All music'], ['chosen', 'Chosen folders']] as const
// Android can't say whether other apps see the synced music, so the last choice is kept here (shown by default).
const OTHER_APPS = 'melue-shown-in-other-apps'

const access = ref<MusicAccess | null>(null)
const covers = ref<FolderCovers | null>(null)
const otherApps = ref(saved() !== 'false')
function saved() {
  try {
    return localStorage.getItem(OTHER_APPS)
  } catch {
    return null
  }
}
const asking = ref(false)
const error = ref('')

onPageLoad(async () => {
  await startLibraryState()
  try {
    ;[access.value, covers.value] = await Promise.all([library.musicAccess(), library.folderCovers()])
  } catch (e) {
    error.value = `Can’t read these settings: ${e}`
  }
})

const allMusic = computed(() => libraryState.folders.find((f) => f.kind === 'android_media'))
const source = computed(() => (allMusic.value ? 'all' : 'chosen'))

async function act(action: () => Promise<unknown>) {
  error.value = ''
  try {
    await action()
  } catch (e) {
    error.value = String(e)
  }
  await refreshLibrary()
}

function pick(next: 'all' | 'chosen') {
  if (next === source.value) return
  if (next === 'chosen') asking.value = true
  else act(() => library.addFolder('android_media', ''))
}
const useChosen = () => act(() => library.removeFolder(allMusic.value!.id))
const chooseFolder = () =>
  act(async () => {
    const location = await storage.pickMusicFolder()
    if (location) await library.addFolder('android_tree', location)
  })
const allow = () => act(async () => (access.value = await library.musicAccess(true)))
const setCovers = (enabled: boolean) => act(async () => (covers.value = await library.setFolderCovers(enabled)))
const setOtherApps = (shown: boolean) =>
  act(async () => {
    await share.setShownInOtherApps(shown)
    otherApps.value = shown
    try {
      localStorage.setItem(OTHER_APPS, String(shown))
    } catch {
      // Not remembered without storage.
    }
  })

const PERMISSION: Record<MusicAccess['permission'], string> = {
  granted: 'Allowed',
  prompt: 'Not allowed yet',
  'prompt-with-rationale': 'Not allowed yet',
  denied: 'Turned down: allow it in Android’s settings (Apps → Melue → Permissions)',
}
</script>

<template>
  <SettingsLabel mark="This device">Music on this phone</SettingsLabel>
  <SettingsCard>
    <SettingsRow
      title="Music from"
      :description="source === 'all' ? 'All music Android knows about, from any app' : 'Only the folders you choose (no permission needed)'"
      wide
    >
      <Segmented :model-value="source" :options="SOURCES" @update:model-value="pick" />
    </SettingsRow>
    <Reveal :show="source === 'all' && !!access">
      <SettingsRow v-if="access" title="Music permission" :description="PERMISSION[access.permission]" detail>
        <Check v-if="access.permission === 'granted'" class="size-[18px] text-ok" />
        <Button v-else-if="access.permission !== 'denied'" size="sm" @click="allow">Allow</Button>
      </SettingsRow>
    </Reveal>
    <Reveal :show="source === 'chosen'">
      <SettingsRow title="Choose a folder" description="It joins your music folders below" detail>
        <Button variant="outline" size="sm" @click="chooseFolder">Choose</Button>
      </SettingsRow>
    </Reveal>
    <SettingsRow v-if="covers" title="Covers from images next to songs" description="folder.jpg, cover.jpg… (asks for the photos permission)">
      <Switch :model-value="covers.enabled" @update:model-value="setCovers" />
    </SettingsRow>
    <SettingsRow v-if="sync.available" title="Show synced music in other apps" description="Other music players see the songs Melue synced">
      <Switch :model-value="otherApps" @update:model-value="setOtherApps" />
    </SettingsRow>
  </SettingsCard>
  <Transition name="drop">
    <SettingsTip v-if="error" kind="bad" class="mt-3">{{ error }}</SettingsTip>
  </Transition>

  <ConfirmDialog
    v-model:open="asking"
    title="Use only chosen folders?"
    text="Songs from the rest of the phone leave your library. The files stay where they are."
    action="Use chosen folders"
    @confirm="useChosen"
  />
</template>
