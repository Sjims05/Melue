<script setup lang="ts">
/**
 * "Show me how it works" for the queue following its list: a playlist and the queue started from it, side by side;
 * the playlist changes step by step (Next change / Back), and the queue shows what the current choices do with it.
 * Change a choice at any step and the queue updates (queueFollow.ts: simulate).
 */
import { ArrowLeft, AudioLines } from '@lucide/vue'
import { computed, ref } from 'vue'
import { Button } from '@/components/ui/button'
import type { QueueFollow } from '@/library'
import { NEW_SONG, simulate, STEPS } from './queueFollow'

const props = defineProps<{ follow: QueueFollow }>()

const step = ref(0)
const LAST = STEPS.length - 1
const shown = computed(() => simulate(props.follow, step.value))
const SONG = 'flex min-h-7 min-w-0 items-center gap-2 rounded-md px-2.5 text-[13px] transition-colors duration-300'
</script>

<template>
  <div class="grid gap-3 rounded-xl border border-dashed border-foreground/20 px-4 py-3.5">
    <div class="flex items-baseline justify-between">
      <b class="text-sm font-semibold text-muted-foreground">Try it</b>
      <small class="text-[12.5px] text-muted-foreground">Step {{ step }} of {{ LAST }}</small>
    </div>
    <Transition name="swap" mode="out-in">
      <p :key="step" class="min-h-[2.6em] text-[13.5px] text-muted-foreground">{{ STEPS[step] }}</p>
    </Transition>
    <div class="grid grid-cols-2 gap-3">
      <div>
        <small class="text-[12.5px] text-muted-foreground">The playlist</small>
        <ol class="mt-1.5 grid gap-[3px]">
          <li
            v-for="title in shown.list"
            :key="title"
            :class="[SONG, title === NEW_SONG && step >= 1 ? 'bg-new text-new-foreground' : 'text-muted-foreground/60']"
          >
            <span class="truncate">{{ title }}</span>
          </li>
        </ol>
      </div>
      <div>
        <small class="text-[12.5px] text-muted-foreground">The queue</small>
        <ol class="mt-1.5 grid gap-[3px]">
          <li
            v-for="entry in shown.queue"
            :key="entry.title"
            :class="[SONG, entry.playing ? 'bg-card font-medium text-foreground' : entry.note ? 'text-muted-foreground' : 'text-muted-foreground/60']"
          >
            <AudioLines v-if="entry.playing" class="size-3.5 flex-none" />
            <span class="flex-1 truncate" :class="{ 'line-through': entry.gone }">{{ entry.title }}</span>
            <em v-if="entry.note" class="rounded-full bg-badge px-[7px] py-px text-[11px] whitespace-nowrap text-soft not-italic">{{ entry.note }}</em>
          </li>
        </ol>
      </div>
    </div>
    <div class="flex justify-between gap-2">
      <Button variant="outline" size="sm" :disabled="step === 0" @click="step--"><ArrowLeft />Back</Button>
      <Button v-if="step < LAST" size="sm" @click="step++">Next change</Button>
      <Button v-else size="sm" @click="step = 0">Start again</Button>
    </div>
  </div>
</template>

