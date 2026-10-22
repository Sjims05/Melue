<script setup lang="ts">
/**
 * Report a bug (lib/report/report.ts): what happened, in the person's words; then the review, the report as it will
 * be sent: names, paths and addresses replaced (each placeholder highlighted; what it stands for is listed, only
 * here), and each part can be left out. Sent as a public issue on Melue's GitHub from the account signed in
 * (Settings: Connections), or kept to send later (offline, or not signed in here: another device signed in sends it
 * through the share), or copied. A screenshot taken for it (when kept) can be dropped here.
 */
import { invoke } from '@tauri-apps/api/core'
import { openUrl } from '@tauri-apps/plugin-opener'
import { ChevronDown } from '@lucide/vue'
import { computed, nextTick, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { Switch } from '@/components/ui/switch'
import { SettingsTip } from '@/components/settings'
import { discardReport, makeReport, noteRest, reportBody, reportTitle, reporting, saveReport, type Report } from '@/lib/report/report'
import { pauseTrail } from '@/lib/report/trail'

const router = useRouter()
const step = ref<'write' | 'review' | 'sent' | 'later'>('write')
const note = ref('')
const report = ref<Report | null>(null)
const busy = ref(false)
const error = ref('')
const copied = ref(false)
const account = ref<string | null>(null)
const issueUrl = ref('')
/** The screenshot stays with the report (dropped: switched off). */
const keepShot = ref(true)
/** Kept (sent or to send): closing doesn't discard it. */
let kept = false
/** Parts shown unfolded in the review. */
const unfolded = ref(new Set<string>())
const field = ref<HTMLTextAreaElement>()

watch(
  () => reporting.open,
  async (open) => {
    if (!open) {
      pauseTrail(false)
      if (!kept) discardReport(reporting.id)
      return
    }
    kept = false
    keepShot.value = true
    step.value = 'write'
    error.value = ''
    report.value = null
    unfolded.value = new Set()
    account.value = await invoke<string | null>('github_account').catch(() => null)
    await nextTick()
    setTimeout(() => field.value?.focus(), 50)
  },
)

async function review() {
  if (!note.value.trim() || !reporting.taken) return
  busy.value = true
  error.value = ''
  try {
    report.value = await makeReport(note.value, reporting.taken)
    step.value = 'review'
  } catch (e) {
    error.value = `Couldn’t make the report: ${e}`
  } finally {
    busy.value = false
  }
}

const title = computed(() => (report.value ? reportTitle(report.value) : ''))
const body = computed(() => (report.value ? reportBody(report.value, reporting.id) : ''))

/** Keeps the report: sent (`sentUrl`), or to send when it can. */
async function keep(sentUrl: string | null) {
  if (reporting.screenshot && !keepShot.value) await discardReport(reporting.id)
  await saveReport({ id: reporting.id, madeMs: reporting.taken?.at ?? Date.now(), title: title.value, body: body.value, sentUrl })
  kept = true
  note.value = ''
}

async function send() {
  busy.value = true
  error.value = ''
  try {
    issueUrl.value = await invoke<string>('github_create_issue', { title: title.value, body: body.value })
    await keep(issueUrl.value)
    step.value = 'sent'
  } catch (e) {
    if (String(e) === 'signed out') {
      account.value = null
      error.value = 'GitHub signed this device out: sign in again in Settings, or send it later'
    } else {
      // Offline, or GitHub not answering: it goes later.
      await sendLater()
    }
  } finally {
    busy.value = false
  }
}

async function sendLater() {
  busy.value = true
  error.value = ''
  try {
    await keep(null)
    step.value = 'later'
  } catch (e) {
    error.value = `Couldn’t keep it: ${e}`
  } finally {
    busy.value = false
  }
}

async function copy() {
  error.value = ''
  try {
    await navigator.clipboard.writeText(`${title.value}\n\n${body.value}`)
    copied.value = true
    setTimeout(() => (copied.value = false), 2000)
  } catch (e) {
    error.value = `Couldn’t copy: ${e}`
  }
}

function signIn() {
  reporting.open = false
  router.push({ path: '/settings', query: { setting: 'connections.github' } })
}

function toggle(id: string) {
  const set = new Set(unfolded.value)
  if (set.has(id)) set.delete(id)
  else set.add(id)
  unfolded.value = set
}

/** The text in pieces, the placeholders marked. */
const PLACEHOLDER = /(\[(?:song|artist|album|playlist|folder|path|share|device|name|link|email|address) \d+\](?:\.\w+)?)/
const pieces = (text: string) => text.split(PLACEHOLDER).map((t, i) => ({ t, mark: i % 2 === 1 }))
</script>

<template>
  <Dialog v-model:open="reporting.open">
    <DialogContent class="max-w-2xl rounded-xl bg-card" @open-auto-focus.prevent>
      <template v-if="step === 'write'">
        <DialogTitle>Report a bug</DialogTitle>
        <DialogDescription>What happened, and what did you expect? Next, you see the whole report before it’s sent.</DialogDescription>
        <textarea
          ref="field"
          v-model="note"
          rows="5"
          aria-label="What happened"
          placeholder="E.g. the album page stayed empty after I added a folder"
          class="w-full resize-none rounded-lg border border-input bg-card px-3 py-2 text-sm outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 dark:bg-background"
          @keydown.ctrl.enter="review"
        />
        <p class="text-[13px] text-muted-foreground">
          The report becomes a public issue on Melue’s GitHub. Song names, paths, addresses and other personal things are replaced first.
        </p>
      </template>

      <template v-else-if="step === 'review' && report">
        <DialogTitle>Your report</DialogTitle>
        <DialogDescription>As it will appear on GitHub. Switch off anything you’d rather not send.</DialogDescription>
        <div class="-mx-2 flex max-h-[58vh] flex-col gap-2 overflow-y-auto px-2">
          <section class="rounded-lg bg-well px-3.5 py-2.5">
            <h3 class="text-[13px] font-semibold">{{ title }}</h3>
            <p v-if="noteRest(report)" class="mt-1 text-[13px] whitespace-pre-line">
              <template v-for="(p, i) in pieces(noteRest(report))" :key="i"><mark v-if="p.mark" class="rounded bg-primary/15 px-0.5 text-foreground">{{ p.t }}</mark><template v-else>{{ p.t }}</template></template>
            </p>
          </section>
          <section v-for="part in report.parts" :key="part.id" class="rounded-lg border" :class="!part.include && 'opacity-55'">
            <div class="flex items-center gap-2 px-3.5 py-2">
              <button type="button" class="flex min-w-0 flex-1 items-center gap-1.5 text-left text-[13px] font-semibold" :aria-expanded="unfolded.has(part.id)" @click="toggle(part.id)">
                <ChevronDown class="size-4 flex-none transition-transform" :class="!unfolded.has(part.id) && '-rotate-90'" />
                {{ part.title }}
              </button>
              <span class="text-xs text-muted-foreground">{{ part.include ? 'Sent' : 'Left out' }}</span>
              <Switch v-model="part.include" :aria-label="`Send ${part.title}`" />
            </div>
            <Transition name="drop">
              <pre
                v-if="unfolded.has(part.id)"
                class="mx-3.5 mb-3 max-h-64 overflow-auto rounded-md bg-well px-3 py-2 font-mono text-[11.5px] leading-relaxed whitespace-pre-wrap select-text"
              ><template v-for="(p, i) in pieces(part.text)" :key="i"><mark v-if="p.mark" class="rounded bg-primary/15 px-0.5 text-foreground">{{ p.t }}</mark><template v-else>{{ p.t }}</template></template></pre>
            </Transition>
          </section>
          <section v-if="reporting.screenshot" class="flex items-center gap-2 rounded-lg border px-3.5 py-2.5" :class="!keepShot && 'opacity-55'">
            <span class="min-w-0 flex-1">
              <span class="block text-[13px] font-semibold">A screenshot of the window</span>
              <span class="block text-xs text-muted-foreground">Kept with the report on your share, for you. Never sent.</span>
            </span>
            <Switch v-model="keepShot" aria-label="Keep the screenshot" />
          </section>
          <section v-if="report.replaced.length" class="rounded-lg border px-3.5 py-2.5">
            <button type="button" class="flex w-full items-center gap-1.5 text-left text-[13px] font-semibold" :aria-expanded="unfolded.has('replaced')" @click="toggle('replaced')">
              <ChevronDown class="size-4 flex-none transition-transform" :class="!unfolded.has('replaced') && '-rotate-90'" />
              What was replaced ({{ report.replaced.length }})
              <span class="ml-auto text-xs font-normal text-muted-foreground">Only shown to you</span>
            </button>
            <Transition name="drop">
              <ul v-if="unfolded.has('replaced')" class="mt-2 flex flex-col gap-1 text-[12.5px]">
                <li v-for="r in report.replaced" :key="r.placeholder" class="flex gap-2">
                  <mark class="flex-none rounded bg-primary/15 px-0.5 font-mono text-[11.5px] text-foreground">{{ r.placeholder }}</mark>
                  <span class="min-w-0 truncate text-muted-foreground">{{ r.original }}</span>
                </li>
              </ul>
            </Transition>
          </section>
        </div>
      </template>

      <template v-else-if="step === 'sent'">
        <DialogTitle>Thanks, it’s sent</DialogTitle>
        <DialogDescription>Your report is an issue on Melue’s GitHub, where you can follow it and add to it.</DialogDescription>
      </template>

      <template v-else-if="step === 'later'">
        <DialogTitle>Thanks, it’s kept to send</DialogTitle>
        <DialogDescription>
          It goes as soon as it can: from this device when it’s online and signed in to GitHub, or from another of your
          devices that is, through your share. About shows when it’s sent.
        </DialogDescription>
      </template>

      <Transition name="drop">
        <SettingsTip v-if="error" kind="bad">{{ error }}</SettingsTip>
      </Transition>

      <div class="flex flex-wrap items-center gap-2 pt-1">
        <template v-if="step === 'write'">
          <Button variant="outline" size="sm" class="ml-auto" @click="reporting.open = false">Cancel</Button>
          <Button size="sm" :disabled="!note.trim() || busy" @click="review">Review</Button>
        </template>
        <template v-else-if="step === 'review'">
          <Button variant="ghost" size="sm" class="mr-auto" @click="step = 'write'">Back</Button>
          <Button variant="outline" size="sm" @click="copy">
            <Transition name="swap" mode="out-in"><span :key="String(copied)">{{ copied ? 'Copied' : 'Copy report' }}</span></Transition>
          </Button>
          <Button v-if="account" size="sm" :disabled="busy" @click="send">Send as {{ account }}</Button>
          <template v-else>
            <Button variant="outline" size="sm" @click="signIn">Sign in to GitHub</Button>
            <Button size="sm" :disabled="busy" @click="sendLater">Send later</Button>
          </template>
        </template>
        <template v-else>
          <Button v-if="step === 'sent'" variant="outline" size="sm" class="ml-auto" @click="openUrl(issueUrl)">Open on GitHub</Button>
          <Button size="sm" :class="step === 'later' && 'ml-auto'" @click="reporting.open = false">Done</Button>
        </template>
      </div>
    </DialogContent>
  </Dialog>
</template>
