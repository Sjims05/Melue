<script setup lang="ts">
/**
 * The share (this device's). Not set up: the form: the address (typed any way, and shown as it was read), user and
 * password, More options (the address at home, where the password is kept), where the synced music goes, Connect.
 * An error shows under the field to fix. Set up: the share (its name shortened in the middle, like the status card;
 * the row opens the full path and the synced folder), with Change password and Remove share in its ⋯ menu.
 */
import { ChevronRight, Cloud, Ellipsis, Folder, Monitor } from '@lucide/vue'
import { computed, reactive, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import {
  Chevron,
  ConfirmDialog,
  EditRow,
  Field,
  IconTile,
  Reveal,
  Segmented,
  SettingsBadge,
  SettingsCard,
  SettingsLabel,
  SettingsRow,
  SettingsTip,
} from '@/components/settings'
import { describeSetupError, share, syncing, type PasswordStore, type SetupError, type ShareInfo, type SyncFolderChoice } from '@/library/sync'
import { isAndroid, isDesktop, storage } from '@/platform'
import { onPageLoad } from '@/components/page/pageLoading'
import { middle, shareName } from './format'
import { refreshLibrary } from './libraryState'

const STORES = [['secure', 'Securely'], ['ask', 'Ask each time'], ['file', 'In a file']] as const
const STORE_LINE: Record<PasswordStore, string> = {
  secure: 'In the system’s secure store',
  ask: 'Only until Melue closes; it asks again after',
  file: 'In a plain file in Melue’s folder: anyone using this device can read it',
}

const info = ref<ShareInfo | null>(null)
const form = reactive({ address: '', user: '', password: '', homeHost: '', store: 'secure' as PasswordStore })
const more = ref(false)
const chosenFolder = ref<SyncFolderChoice | null>(null)
const connecting = ref(false)
const errors = reactive({ address: '', user: '', password: '', store: '', general: '' })
const connected = ref(false)
const changingPassword = ref(false)
const removing = ref(false)
const details = ref(false)
const error = ref('')

onPageLoad(async () => {
  try {
    info.value = await share.info()
  } catch (e) {
    error.value = `Can’t read the share: ${describeSetupError(e)}`
  }
})

/** How the address was read: \\nas\music\Rock, smb://nas/music or nas/music all give nas → music → Rock. */
const parsed = computed(() => {
  const parts = form.address.trim().replace(/^smb:\/\//i, '').replace(/^\\\\/, '').replace(/\\/g, '/').split('/').filter(Boolean)
  return { host: parts[0] ?? '', share: parts[1] ?? '', folder: parts.slice(2).join('/') }
})

// Where the synced music goes: desktop Music/Melue unless another folder is chosen; on a phone, a chosen folder.
const syncFolder = computed<SyncFolderChoice | null>(
  () => chosenFolder.value ?? (info.value?.defaultSyncFolder ? { kind: 'path', location: info.value.defaultSyncFolder } : null),
)
async function chooseFolder() {
  const location = await storage.pickMusicFolder()
  if (location) chosenFolder.value = { kind: isAndroid ? 'android_tree' : 'path', location }
}

/** A setup error, under the field to fix (or under the form). */
function showError(e: unknown) {
  Object.assign(errors, { address: '', user: '', password: '', store: '', general: '' })
  const failure = e as SetupError
  if (failure?.kind === 'share') {
    const s = failure.error
    const host = parsed.value.host
    if (s.kind === 'unreachable') return (errors.address = `Can’t reach “${host}”. Check that it’s on and that this device can reach it.`)
    if (s.kind === 'notFound') return (errors.address = `Reached ${host}, but found no “${s.path}” there.`)
    if (s.kind === 'login') return (errors.password = 'The NAS turned down this user or password.')
    if (s.kind === 'noPermission') return (errors.user = `This user isn’t allowed into “${s.path}”.`)
  }
  if (failure?.kind === 'noSecureStore') {
    more.value = true
    return (errors.store = 'This device has no secure store for passwords: pick Ask each time or In a file.')
  }
  errors.general = describeSetupError(e)
}

async function connect() {
  const { host, share: name, folder } = parsed.value
  if (!host || !name) return (errors.address = 'Type the NAS and the share, like nas/music.')
  if (!syncFolder.value) return showError({ kind: 'other', detail: 'Choose where the synced music goes.' })
  connecting.value = true
  try {
    const setup = { kind: 'smb' as const, host, share: name, folder, user: form.user.trim(), passwordStore: form.store, homeHost: form.homeHost.trim() || undefined }
    info.value = await share.setUp(setup, form.password, syncFolder.value)
    Object.assign(errors, { address: '', user: '', password: '', store: '', general: '' })
    form.password = ''
    connected.value = true
    await refreshLibrary()
  } catch (e) {
    showError(e)
  } finally {
    connecting.value = false
  }
}

async function changePassword(password: string) {
  error.value = ''
  try {
    info.value = await share.setPassword(password)
    changingPassword.value = false
  } catch (e) {
    error.value = describeSetupError(e)
  }
}

async function remove() {
  error.value = ''
  try {
    info.value = await share.remove()
    connected.value = false
    await refreshLibrary()
  } catch (e) {
    error.value = describeSetupError(e)
  }
}

async function startSync() {
  connected.value = false
  await syncing.now().catch((e) => (error.value = String(e)))
}

const settings = computed(() => info.value?.settings ?? null)

// The whole row opens the details, except its buttons (the ⋯ menu).
function toggleDetails(event: Event) {
  if (!(event.target as HTMLElement).closest('button')) details.value = !details.value
}
</script>

<template>
  <template v-if="info">
    <!-- Not set up: the form. -->
    <template v-if="!settings">
      <SettingsTip v-if="isDesktop" class="mb-3">
        Have your NAS as a drive in Windows (like Z:)? Add it as a music folder to stream straight from it, or set it up below to sync it.
      </SettingsTip>
      <SettingsLabel mark="This device">Your share</SettingsLabel>
      <SettingsCard>
        <div class="settings-row grid gap-3" data-row>
          <Field v-model="form.address" label="Address" placeholder="\\nas\music or nas/music" :error="errors.address" />
          <!-- How the address was read. -->
          <div v-if="!errors.address" class="flex flex-wrap items-center gap-1.5 text-muted-foreground">
            <span class="inline-flex h-7 items-center gap-1.5 rounded-[7px] bg-well px-2.5 text-[13px] text-foreground shadow-[inset_0_0_0_1px_var(--well-line)]">
              <Monitor class="size-[13px]" />{{ parsed.host || '…' }}
            </span>
            <ChevronRight class="size-[13px]" />
            <span class="inline-flex h-7 items-center gap-1.5 rounded-[7px] bg-well px-2.5 text-[13px] text-foreground shadow-[inset_0_0_0_1px_var(--well-line)]">
              <Folder class="size-[13px]" />{{ parsed.share || 'share?' }}
            </span>
            <ChevronRight class="size-[13px]" />
            <span class="inline-flex h-7 items-center rounded-[7px] bg-well px-2.5 text-[13px] text-foreground shadow-[inset_0_0_0_1px_var(--well-line)]">
              {{ parsed.folder || 'whole share' }}
            </span>
          </div>
          <div class="grid grid-cols-2 gap-3">
            <Field v-model="form.user" label="User" :error="errors.user" />
            <Field v-model="form.password" label="Password" type="password" :error="errors.password" />
          </div>
        </div>
        <SettingsRow as="button" title="More options" detail :aria-expanded="more" @click="more = !more">
          <template #end><Chevron :open="more" /></template>
        </SettingsRow>
        <Reveal :show="more">
          <div class="settings-row detail grid" data-row>
            <Field v-model="form.homeHost" label="Address on home network" placeholder="optional: 192.168.0.20" hint="Tried first at home, so a VPN hiccup doesn’t matter" />
          </div>
          <SettingsRow title="Keep the password" detail wide>
            <template #description>
              <span :class="errors.store && 'text-destructive'">{{ errors.store || STORE_LINE[form.store] }}</span>
            </template>
            <Segmented v-model="form.store" :options="STORES" />
          </SettingsRow>
        </Reveal>
        <SettingsRow title="Synced music goes to" :description="syncFolder ? middle(syncFolder.location, 44) : 'Choose a folder on this phone'">
          <Button variant="outline" size="sm" @click="chooseFolder">{{ syncFolder ? 'Change' : 'Choose folder' }}</Button>
        </SettingsRow>
        <div class="settings-row" data-row>
          <Button :disabled="connecting" @click="connect">{{ connecting ? 'Connecting…' : 'Connect to share' }}</Button>
        </div>
      </SettingsCard>
      <SettingsTip v-if="errors.general" kind="bad" class="mt-3">{{ errors.general }}</SettingsTip>
    </template>

    <!-- Set up: the share and where its music goes. -->
    <template v-else>
      <SettingsLabel mark="This device">Your share</SettingsLabel>
      <SettingsTip v-if="connected" kind="ok" class="mb-3">
        Connected to {{ shareName(settings) }}
        <template #action><Button size="sm" @click="startSync">Start sync</Button></template>
      </SettingsTip>
      <SettingsCard>
        <SettingsRow
          :description="info.syncFolder ? middle(info.syncFolder.location, 44) : undefined"
          :aria-expanded="details"
          class="cursor-pointer"
          @click="toggleDetails"
        >
          <template #lead><IconTile :icon="Cloud" /></template>
          <template #title>
            {{ middle(shareName(settings)) }}
            <SettingsBadge class="ml-1.5">{{ settings.kind === 'drive' && settings.stream ? 'Streamed' : 'Synced' }}</SettingsBadge>
          </template>
          <DropdownMenu>
            <DropdownMenuTrigger as-child>
              <Button variant="ghost" size="icon-sm" aria-label="The share: more"><Ellipsis /></Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem v-if="settings.kind === 'smb'" @select="changingPassword = !changingPassword">Change password</DropdownMenuItem>
              <DropdownMenuItem variant="destructive" @select="removing = true">Remove share…</DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
          <template #end><Chevron :open="details" /></template>
        </SettingsRow>
        <!-- The details: the full paths, behind the card's details line (like the status card). -->
        <Reveal :show="details" :line="false">
          <div class="mx-4 grid grid-cols-[auto_1fr] gap-x-7 gap-y-2 pb-3 text-[13.5px]">
            <div class="settings-details-line col-span-2 mb-1" />
            <span class="font-medium">Share</span>
            <span class="break-all text-muted-foreground">{{ shareName(settings) }}</span>
            <template v-if="info.syncFolder">
              <span class="font-medium">Synced to</span>
              <span class="break-all text-muted-foreground">{{ info.syncFolder.location }}</span>
            </template>
          </div>
        </Reveal>
        <Reveal :show="changingPassword">
          <EditRow title="New password" type="password" @save="changePassword" @cancel="changingPassword = false" />
        </Reveal>
      </SettingsCard>
    </template>
  </template>
  <SettingsTip v-if="error" kind="bad" class="mt-3">{{ error }}</SettingsTip>

  <ConfirmDialog
    v-model:open="removing"
    title="Remove the share?"
    text="Syncing stops on this device. The synced folder stays, with its songs."
    action="Remove share"
    danger
    @confirm="remove"
  />
</template>
