<script setup lang="ts">
/**
 * Search (opened from the top bar's field, Ctrl Space or "/"): its own big field, focused, and what's typed kept in the
 * address (?q=), so going back to it shows the same search. Escape empties the field, then closes Search.
 */
import { Search, X } from '@lucide/vue'
import { onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { PageHeader, PlannedCard } from '@/components/page'
import { goBack } from '@/router'
import { LINK } from '@/settings/settingsMenu'

const route = useRoute()
const router = useRouter()
const field = ref<HTMLInputElement>()
const q = ref(typeof route.query.q === 'string' ? route.query.q : '')

// Focused once it has slid in (focusing while it moves would scroll it).
onMounted(() => field.value?.focus({ preventScroll: true }))
const typed = () => router.replace({ query: q.value ? { q: q.value } : {} })
// A setting's link pasted here (melue://settings/sync.atStart), Enter: Settings, at that setting.
function enter() {
  const text = q.value.trim()
  if (text.toLowerCase().startsWith(LINK)) router.push({ path: '/settings', query: { setting: text.slice(LINK.length) } })
}
function escape() {
  if (q.value) {
    q.value = ''
    typed()
  } else goBack()
}
</script>

<template>
  <!-- One root: page switches move the page as one. -->
  <div>
    <PageHeader title="Search" subtitle="Find anything in the library" />
    <label class="mb-8 flex h-[52px] items-center gap-3 rounded-xl border border-border bg-card px-4 dark:border-transparent">
      <Search class="size-5 flex-none text-muted-foreground" />
      <input
        ref="field"
        v-model="q"
        type="search"
        placeholder="Songs, albums, artists, playlists"
        class="min-w-0 flex-1 bg-transparent text-[15px] outline-none placeholder:text-muted-foreground"
        @input="typed"
        @keydown.esc.prevent="escape"
        @keydown.enter.prevent="enter"
      />
      <button
        v-if="q"
        type="button"
        class="grid size-8 flex-none place-items-center rounded-md text-muted-foreground transition-colors hover:bg-well hover:text-foreground"
        aria-label="Clear"
        @click="(q = ''), typed(), field?.focus()"
      >
        <X class="size-4" />
      </button>
    </label>
    <PlannedCard
      :title="q ? `Results for “${q}”` : 'Recently added songs'"
      text="Results grouped as songs, albums, artists and playlists, and recently added songs: a coming step (Home and Search)."
    />
  </div>
</template>
