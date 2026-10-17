<script setup lang="ts">
/**
 * The sync status card, at the top of the settings: the one live status for syncing (Melue's design, with the states
 * from the lab). Not set up, password needed, syncing (with a bar and Cancel), can't reach (with when it tries again),
 * synced; the chevron opens the last sync's details, what waits for an answer, and the address at home.
 *
 * Reads Melue's live sync status (library/sync.ts) and the share's info.
 */
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Button } from '@/components/ui/button'
import { Chevron, EditRow, Reveal, SettingsCard, SettingsRow, StatusDot } from '@/components/settings'
import { describeSetupError, describeSyncFailure, share, sync, syncing, type ShareInfo, type SyncProgress } from '@/library/sync'
import { ago, at, didWords, middle, shareName, size, TRIGGER, when } from './format'

const emit = defineEmits<{ setUp: [] }>()

const info = ref<ShareInfo | null>(null)
const loadInfo = async () => {
  info.value = await share.info().catch(() => null)
}
onMounted(loadInfo)
watch(() => sync.available, loadInfo)

const open = ref(false)
const editing = ref<'' | 'password' | 'home'>('')
const error = ref('')
const saving = ref(false)

const status = computed(() => sync.status)

// What the card shows of a running sync: held at least SHOWN_AT_LEAST ms, and when it's done the bar fills and stays a
// moment (FILLED_FOR) before "Synced", so a quick sync reads as one, not a flash. Only what's shown: syncing isn't slowed.
const SHOWN_AT_LEAST = 1200
const FILLED_FOR = 400
const shownProgress = ref<SyncProgress | null>(null)
let shownSince = 0
let hide: ReturnType<typeof setTimeout> | undefined
watch(
  () => status.value?.syncing ?? null,
  (live) => {
    clearTimeout(hide)
    if (live) {
      if (!shownProgress.value) shownSince = Date.now()
      shownProgress.value = live
      return
    }
    const shown = shownProgress.value
    if (!shown) return
    shownProgress.value = { ...shown, filesDone: shown.filesTotal, bytesDone: shown.bytesTotal, current: null }
    hide = setTimeout(() => (shownProgress.value = null), Math.max(0, SHOWN_AT_LEAST - (Date.now() - shownSince)) + FILLED_FOR)
  },
  { immediate: true },
)
onBeforeUnmount(() => clearTimeout(hide))
const settings = computed(() => info.value?.settings ?? null)
const streamed = computed(() => settings.value?.kind === 'drive' && settings.value.stream)
// The full path shows in the details; the title shortens it in the middle when it's long.
const fullName = computed(() => shareName(settings.value))
const name = computed(() => `${middle(fullName.value)}${streamed.value ? ' · Streamed' : ''}`)
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

const dot = computed(() => ({ none: 'off', password: 'bad', syncing: 'busy', problem: 'bad', synced: 'ok' } as const)[state.value])
const canOpen = computed(() => ['syncing', 'problem', 'synced'].includes(state.value))
// The address at home: only for a share Melue signs in to, and only while there isn't one.
const canAddHome = computed(() => settings.value?.kind === 'smb' && !settings.value.homeHost)

// The whole row opens the details (like any row that opens something), except clicks on its buttons.
function toggle(event: Event) {
  if (!canOpen.value || (event.target as HTMLElement).closest('button')) return
  open.value = !open.value
}

async function syncNow(confirmedRemoval = false) {
  error.value = ''
  try {
    await syncing.now(confirmedRemoval)
  } catch (e) {
    error.value = describeSyncFailure(e)
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
      <div class="h-1 overflow-hidden rounded-full bg-secondary">
        <div class="h-full rounded-full bg-primary transition-[width] duration-300" :style="{ width: `${percent}%` }" />
      </div>
    </div>

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

    <!-- The details: the last sync, what waits for an answer, notes about the connection. -->
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
        <div v-if="status?.last?.result.removalsWaiting" class="mt-3 flex flex-wrap items-center gap-3">
          <span class="flex-1">{{ status.last.result.removalsWaiting }} songs are gone from the share: remove them here too?</span>
          <Button variant="outline" size="sm" @click="syncNow(true)">Remove them</Button>
        </div>
        <p v-if="status?.note" class="mt-3 text-muted-foreground">{{ status.note }}</p>
        <p v-if="error && !editing" class="mt-3 text-destructive">{{ error }}</p>
      </div>
    </Reveal>
  </SettingsCard>
</template>
