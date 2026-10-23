<script setup lang="ts">
/**
 * The sync status card, at the top of the settings: the one live status for syncing (Melue's design, with the states
 * from the lab). Not set up, password needed, syncing (with a bar and Cancel), can't reach (with when it tries again),
 * synced; the chevron opens the last sync's details, what waits for an answer, and the address at home.
 *
 * Reads Melue's live sync status (library/sync.ts) and the share's info.
 */
import { computed, ref, watch } from 'vue'
import { Button } from '@/components/ui/button'
import { Chevron, EditRow, Reveal, SettingsCard, SettingsRow, StatusDot } from '@/components/settings'
import { describeSetupError, describeSyncFailure, share, sync, syncing, type ShareInfo, type SyncFailure } from '@/library/sync'
import { onPageLoad } from '@/components/page/pageLoading'
import { ago, at, didWords, lastPart, middle, shareName, size, TRIGGER, when } from './format'
import { count } from '@/lib/format'
import { usePhone } from '@/lib/phone'
import { useShownSync } from './shownSync'

// On a phone the share shows by its last folder (the details have all of it).
const phone = usePhone()

const emit = defineEmits<{ setUp: [] }>()

const info = ref<ShareInfo | null>(null)
const loadInfo = async () => {
  info.value = await share.info().catch(() => null)
}
onPageLoad(loadInfo)
watch(() => sync.available, loadInfo)

const open = ref(false)
const editing = ref<'' | 'password' | 'home'>('')
const error = ref('')
const saving = ref(false)

const status = computed(() => sync.status)

// What the card shows of a running sync: held a moment, so a quick sync reads as one (shownSync.ts).
const shownProgress = useShownSync()
const settings = computed(() => info.value?.settings ?? null)
const streamed = computed(() => settings.value?.kind === 'drive' && settings.value.stream)
// The full path shows in the details; the title shortens it in the middle when it's long.
const fullName = computed(() => shareName(settings.value))
// On a phone just its last folder (the details show all of it).
const name = computed(() => `${phone ? lastPart(fullName.value) : middle(fullName.value)}${streamed.value ? ' · Streamed' : ''}`)
const unreachable = computed(() => {
  const problem = status.value?.problem
  return problem?.kind === 'setup' && problem.error.kind === 'share' && problem.error.error.kind === 'unreachable'
})

type State = 'none' | 'password' | 'syncing' | 'problem' | 'synced'
const state = computed<State>(() => {
  if (!sync.available || !settings.value) return 'none'
  if (info.value?.needsPassword) return 'password'
  if (shownProgress.value) return 'syncing'
  if (status.value?.problem) return 'problem'
  return 'synced'
})

const progress = computed(() => shownProgress.value)
const percent = computed(() => {
  const p = progress.value
  if (!p) return 0
  return p.bytesTotal ? (p.bytesDone / p.bytesTotal) * 100 : p.filesTotal ? (p.filesDone / p.filesTotal) * 100 : 0
})

const title = computed(() => {
  switch (state.value) {
    case 'none': return 'No share set up'
    case 'password': return 'Password needed'
    case 'syncing': return `${name.value} · Syncing`
    case 'problem': return `${name.value} · ${unreachable.value ? 'Can’t reach the share' : 'Couldn’t sync'}`
    default: return streamed.value ? name.value : `${name.value} · Synced`
  }
})

const description = computed(() => {
  const s = status.value
  switch (state.value) {
    case 'none': return 'Your music on every device: set one up under Library'
    case 'password': return 'The share asks for its password since Melue restarted'
    case 'syncing': {
      const p = progress.value!
      const file = p.current ? ` · ${p.current.split('/').pop()}` : ''
      return `Copying ${p.filesDone} of ${p.filesTotal} songs · ${size(p.bytesDone)} of ${size(p.bytesTotal)}${file}`
    }
    case 'problem': {
      const retry = s?.retryAtMs ? ` Trying again at ${at(s.retryAtMs)}.` : ''
      return unreachable.value ? `Check that it’s on and that this device can reach it.${retry}` : `${describeSyncFailure(s?.problem)}.${retry}`
    }
    default: {
      if (!s?.last) return 'Not synced yet'
      return `${streamed.value ? 'Likes and playlists synced' : 'Synced'} ${ago(s.last.atMs)} · ${didWords(s.last.result)}`
    }
  }
})

/** Songs gone from the share that wait for a yes (more than Ask first allows). */
const removalsWaiting = computed(() => status.value?.last?.result.removalsWaiting ?? 0)

const dot = computed(() => ({ none: 'off', password: 'bad', syncing: 'busy', problem: 'bad', synced: 'ok' } as const)[state.value])
const canOpen = computed(() => ['syncing', 'problem', 'synced'].includes(state.value))
// The address at home: only for a share Melue signs in to, and only while there isn't one.
const canAddHome = computed(() => settings.value?.kind === 'smb' && !settings.value.homeHost)

// The whole row opens the details (like any row that opens something), except clicks on its buttons.
function toggle(event: Event) {
  if (!canOpen.value || (event.target as HTMLElement).closest('button')) return
  open.value = !open.value
}

const mobileOffer = computed(() => sync.status?.mobileOffer ?? null)
async function syncNow(confirmedRemoval = false, useMobileData = false) {
  error.value = ''
  try {
    await syncing.now(confirmedRemoval, useMobileData)
  } catch (e) {
    if ((e as SyncFailure)?.kind !== 'cancelled') error.value = describeSyncFailure(e)
  }
}

async function save(value: string) {
  saving.value = true
  error.value = ''
  try {
    info.value = editing.value === 'password' ? await share.setPassword(value) : await share.setHomeHost(value)
    editing.value = ''
  } catch (e) {
    error.value = describeSetupError(e)
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <SettingsCard>
    <SettingsRow
      stack
      :title="title"
      :description="description"
      :class="canOpen && 'cursor-pointer'"
      :role="canOpen ? 'button' : undefined"
      :tabindex="canOpen ? 0 : undefined"
      :aria-expanded="canOpen ? open : undefined"
      @click="toggle"
      @keydown.enter.self="toggle"
      @keydown.space.self.prevent="toggle"
    >
      <template #lead><StatusDot :kind="dot" /></template>
      <Transition name="drop">
        <Button v-if="open && canAddHome && !editing" variant="outline" size="sm" @click="editing = 'home'">Add home address</Button>
      </Transition>
      <Button v-if="state === 'none'" size="sm" @click="emit('setUp')">Set up</Button>
      <Button v-else-if="state === 'password' && !editing" size="sm" @click="editing = 'password'">Give password</Button>
      <Button v-else-if="state === 'syncing'" variant="outline" size="sm" @click="syncing.cancel()">Cancel</Button>
      <Button v-else-if="state !== 'password'" variant="outline" size="sm" @click="syncNow()">{{ state === 'problem' ? 'Try now' : 'Sync now' }}</Button>
      <template v-if="canOpen" #end>
        <Chevron :open="open" />
      </template>
    </SettingsRow>

    <div v-if="progress" class="-mt-1.5 pr-4 pb-3.5 pl-10">
      <div class="h-1 overflow-hidden rounded-full bg-well shadow-[inset_0_0_0_1px_var(--well-line)]">
        <div class="h-full rounded-full bg-primary transition-[width] duration-300" :style="{ width: `${percent}%` }" />
      </div>
    </div>

    <!-- On mobile data, songs that weren't copied yet (too big for a sync by itself, or the month's limit). -->
    <Reveal :show="!!mobileOffer && state !== 'syncing'">
      <SettingsRow
        :title="`${size(mobileOffer?.bytes ?? 0)} of songs waiting for Wi-Fi`"
        :description="mobileOffer?.reason === 'limit' ? 'This month’s mobile data limit is reached' : 'More than you set Melue to copy on mobile data by itself'"
        detail
      >
        <Button variant="outline" size="sm" @click="syncNow(false, true)">Copy on mobile data</Button>
      </SettingsRow>
    </Reveal>
    <!-- Songs gone from the share, more than Melue removes without asking: the question, in sight (not in the details). -->
    <Reveal :show="!!removalsWaiting && state !== 'syncing'">
      <SettingsRow
        :title="`${count(removalsWaiting, 'song')} gone from the share`"
        :description="`More than Melue removes by itself (Ask first). Remove ${removalsWaiting === 1 ? 'it' : 'them'} here too?`"
        detail
      >
        <Button variant="outline" size="sm" @click="syncNow(true)">{{ removalsWaiting === 1 ? 'Remove it' : 'Remove them' }}</Button>
      </SettingsRow>
    </Reveal>
    <Reveal :show="editing === 'password'">
      <EditRow
        title="The share’s password"
        type="password"
        :error="error"
        :saving="saving"
        @save="save"
        @cancel="editing = ''; error = ''"
      />
    </Reveal>
    <Reveal :show="editing === 'home'">
      <EditRow
        title="Address at home"
        description="Tried first when you’re at home: an address or a name"
        placeholder="192.168.0.20 or nas.local"
        hint="Tip: give your NAS a fixed address in your router, or use its name."
        :error="error"
        :saving="saving"
        @save="save"
        @cancel="editing = ''; error = ''"
      />
    </Reveal>

    <!-- The details: the last sync, notes about the connection. -->
    <Reveal :show="open && canOpen" :line="false">
      <div class="mx-4 pb-3 text-[13.5px]">
        <div class="settings-details-line mb-3" />
        <p class="mb-3 break-all text-muted-foreground">{{ fullName }}</p>
        <div v-if="status?.last" class="grid grid-cols-[auto_1fr] gap-x-7 gap-y-2">
          <span class="font-medium">{{ when(status.last.atMs) }}</span>
          <span class="text-muted-foreground">{{ TRIGGER[status.last.trigger] }} · {{ didWords(status.last.result) }}</span>
          <template v-if="status.last.result.stopped">
            <span />
            <span class="text-destructive">Stopped: {{ status.last.result.stopped }}</span>
          </template>
          <template v-if="status.last.result.failed.length">
            <span />
            <span class="text-destructive">{{ status.last.result.failed.length }} couldn’t be copied: tried again next sync</span>
          </template>
          <template v-if="status.last.result.waitingForWifi">
            <span />
            <span class="text-muted-foreground">{{ status.last.result.waitingForWifi }} songs wait for Wi-Fi</span>
          </template>
        </div>
        <div v-else class="text-muted-foreground">Not synced yet</div>
        <p v-if="status?.note" class="mt-3 text-muted-foreground">{{ status.note }}</p>
        <p v-if="error && !editing" class="mt-3 text-destructive">{{ error }}</p>
      </div>
    </Reveal>
  </SettingsCard>
</template>
