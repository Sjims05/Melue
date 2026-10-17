<script setup lang="ts">
/**
 * The spike's Components page: the settings' building blocks in Melue's design (the playground's component lab has
 * them all). The finished settings are on the Settings page (settings/SettingsPage.vue).
 */
import { ChevronRight, Copy, Folder, HardDrive, Cloud, Link, Lock } from '@lucide/vue'
import { computed, reactive, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { AUTO_COVERS, CoverBadge, CoverCard, coverMenu } from '@/components/covers'
import { SongList, songMenu, type Song, type SongAction } from '@/components/songs'
import { Checkbox } from '@/components/ui/checkbox'
import { Switch } from '@/components/ui/switch'
import {
  Chips,
  ConfirmDialog,
  DropValue,
  Field,
  Hint,
  IconTile,
  RadioMark,
  Segmented,
  SettingsBadge,
  SettingsCard,
  SettingsHeading,
  SettingsLabel,
  SettingsRow,
  SettingsTip,
  StatusDot,
  Stepper,
} from '@/components/settings'
import { themeMode } from '@/lib/theme'
import { SAMPLE_SONGS } from './sampleSongs'

// The building blocks' demo values (nothing here is a real setting).
const demo = reactive({
  on: true,
  chips: 15,
  segment: 'quadratic' as 'quadratic' | 'cubic' | 'linear',
  limit: 250 as number | 'off',
  steps: 5,
  device: 'system',
  address: 'nas/music',
  picked: [true, false] as boolean[],
  asking: false,
})
// The song lists' demo: a queue and the same songs A–Z, one song playing; what the menus pick shows under them.
const queue = ref<Song[]>(SAMPLE_SONGS.slice(0, 7).map((s) => ({ ...s })))
const all = ref<Song[]>(SAMPLE_SONGS.map((s) => ({ ...s })))
const library = computed(() => [...all.value].sort((a, b) => a.title.localeCompare(b.title)))
const now = reactive({ id: 1 as Song['id'] | null, playing: true })
const picked = ref('Right-click a song, or use its ⋯ menu.')
const ACTION_WORDS: Record<SongAction, string> = {
  playNext: 'Play next', addToQueue: 'Add to queue', like: 'Add to liked', goToAlbum: 'Go to album', goToArtist: 'Go to artist',
  addToPlaylist: 'Add to playlist', showInFolder: 'Show in folder', moveToSync: 'Move to sync', removeFromPlaylist: 'Remove from playlist',
}
function play(song: Song) {
  now.id = song.id
  now.playing = true
}
function like(song: Song, liked: boolean) {
  for (const list of [queue.value, all.value]) {
    const found = list.find((s) => s.id === song.id)
    if (found) found.liked = liked
  }
}
function move(from: number, to: number) {
  const [song] = queue.value.splice(from, 1)
  queue.value.splice(to, 0, song)
}

// The covers' demo: playlists (a collage of their songs' covers, with what kind they are), Made for you, an album,
// recently added songs.
const collage = (from: number) => [0, 1, 2, 3].map((i) => SAMPLE_SONGS[(from + i) % 7].cover!)
const PLAYLISTS = [
  { title: 'Night Drive', subtitle: '8 songs · 31 min', from: 0, lives: 'synced', badge: { icon: Cloud, label: 'Synced' } },
  { title: 'Sunday morning', subtitle: '12 songs · 44 min', from: 4, lives: 'synced', badge: { icon: Cloud, label: 'Synced' } },
  { title: 'Just mine', subtitle: '8 songs · 1 unavailable · 31 min', from: 2, lives: 'here', badge: { icon: Lock, label: 'Only on this device' } },
  { title: 'Road trip', subtitle: '31 songs · 2 h 4 min', from: 5, lives: 'linked', badge: { icon: Link, label: 'Linked' } },
  { title: 'Focus', subtitle: '20 songs · 1 h 12 min', from: 1, lives: 'synced', badge: { icon: Cloud, label: 'Synced' } },
] as const
// What a card's menu picked, in words (shown under the song lists).
const cardPicked = (title: string, action: string) => {
  const item = [...coverMenu({ kind: 'playlist', lives: 'synced' }), ...coverMenu({ kind: 'playlist', lives: 'here' }), ...coverMenu({ kind: 'playlist', lives: 'linked' }), ...coverMenu({ kind: 'album' }), ...coverMenu({ kind: 'auto' }), ...songMenu(SAMPLE_SONGS[0])]
    .find((i) => i.action === action)
  picked.value = `${item?.label ?? action}: ${title}`
}
const MADE_FOR_YOU = [
  { title: 'All songs', subtitle: '1.234 songs', ...AUTO_COVERS.songs },
  { title: 'Liked songs', subtitle: '86 songs · 5 h 12 min', ...AUTO_COVERS.liked },
  { title: 'Top songs', subtitle: 'All time · 50 songs', ...AUTO_COVERS.topAllTime },
  { title: 'Top songs', subtitle: 'Last year · 50 songs', ...AUTO_COVERS.topYear },
  { title: 'Top songs', subtitle: 'Last 6 months · 50 songs', ...AUTO_COVERS.topHalfYear },
  { title: 'Top songs', subtitle: 'Last month · 50 songs', ...AUTO_COVERS.topMonth },
  { title: 'Synced music', subtitle: 'Folder · 218 songs', ...AUTO_COVERS.folder },
]

const DEVICES = [
  ['system', 'System default', 'Follows Windows’ choice · now Speakers (Realtek)'],
  ['sonar', 'SteelSeries Sonar - Media', ''],
  ['speakers', 'Speakers (Realtek)', ''],
] as const
</script>

<template>
  <main class="mx-auto max-w-[960px] px-4 pt-6 pb-16">
    <header class="mb-8 flex flex-wrap items-end justify-between gap-4">
      <div>
        <h1 class="text-[28px] font-bold tracking-tight">Components</h1>
        <p class="mt-1 text-sm text-muted-foreground">The building blocks every setting is made of. The settings are on the Settings page.</p>
      </div>
      <Segmented v-model="themeMode" :options="[['system', 'System'], ['dark', 'Dark'], ['light', 'Light']]" />
    </header>

    <SettingsHeading title="Song lists" description="The queue (drag the handles, ✕ takes a song out) and the library A–Z (two songs only on this device). Hover, play, like, right-click." />
    <SettingsLabel>Up next · {{ queue.length }} songs</SettingsLabel>
    <SongList
      :songs="queue"
      queue
      :current-id="now.id"
      :playing="now.playing"
      plays-next
      :place="{ showInFolder: true, moveToSync: true }"
      @play="play"
      @pause="now.playing = false"
      @like="like"
      @remove="(_, index) => queue.splice(index, 1)"
      @move="move"
      @action="(song, action) => (picked = `${ACTION_WORDS[action]}: ${song.title}`)"
    />
    <SettingsLabel>Library A–Z</SettingsLabel>
    <SongList
      :songs="library"
      az
      :current-id="now.id"
      :playing="now.playing"
      :unavailable="[8]"
      :local-only="[3, 7]"
      :place="{ showInFolder: true }"
      @play="play"
      @pause="now.playing = false"
      @like="like"
      @action="(song, action) => (picked = `${ACTION_WORDS[action]}: ${song.title}`)"
    />
    <p class="mt-3 mb-14 text-[13px] text-muted-foreground">{{ picked }}</p>

    <SettingsHeading title="Covers" description="Playlists, albums, auto playlists and songs as cards. Hover a playlist for what its corner chip means; right-click a card." />
    <p class="mb-3 text-[13px] text-muted-foreground">{{ picked }}</p>
    <SettingsLabel>Your playlists</SettingsLabel>
    <div class="cover-grid" style="--cover-min: 180px">
      <CoverCard
        v-for="p in PLAYLISTS"
        :key="p.title"
        :title="p.title"
        :subtitle="p.subtitle"
        :collage="collage(p.from)"
        :menu="coverMenu({ kind: 'playlist', lives: p.lives })"
        @action="(a) => cardPicked(p.title, a)"
      >
        <template #badge><CoverBadge :icon="p.badge.icon" :label="p.badge.label" /></template>
      </CoverCard>
    </div>
    <SettingsLabel>Made for you</SettingsLabel>
    <div class="cover-grid">
      <CoverCard
        v-for="p in MADE_FOR_YOU"
        :key="p.subtitle"
        :title="p.title"
        :subtitle="p.subtitle"
        :colors="p.colors"
        :icon="p.icon"
        :menu="coverMenu({ kind: 'auto' })"
        @action="(a) => cardPicked(p.title, a)"
      />
    </div>
    <SettingsLabel>Albums and songs</SettingsLabel>
    <div class="cover-grid mb-14">
      <CoverCard
        title="Northbound"
        subtitle="Juniper Vale · 2023"
        :colors="['#2fa58f', '#9be8cf']"
        :menu="coverMenu({ kind: 'album' })"
        @action="(a) => cardPicked('Northbound', a)"
      />
      <CoverCard
        v-for="s in SAMPLE_SONGS.slice(0, 6)"
        :key="s.id"
        :title="s.title"
        :subtitle="s.artist"
        :image="s.cover"
        :menu="songMenu(s, { showInFolder: true })"
        @action="(a) => cardPicked(s.title, a)"
      />
    </div>

    <SettingsHeading title="Building blocks" description="What every settings component is made of (Melue’s design)." />

    <SettingsLabel>Rows and controls</SettingsLabel>
    <SettingsCard>
      <SettingsRow title="A switch" description="Gets new music as soon as you open it">
        <Switch v-model="demo.on" />
      </SettingsRow>
      <SettingsRow title="Chips" description="One of a few values" wide>
        <Chips v-model="demo.chips" :options="[[0, 'Never'], [5, '5 min'], [15, '15 min'], [60, '1 hour']]" />
      </SettingsRow>
      <SettingsRow title="Segments" description="One of a few choices" wide>
        <Segmented v-model="demo.segment" :options="[['quadratic', 'Quadratic'], ['cubic', 'Cubic'], ['linear', 'Linear']]" />
      </SettingsRow>
      <SettingsRow title="A detail row with a dropdown value" description="Per month; then songs wait for Wi-Fi" detail>
        <DropValue v-model="demo.limit" :options="[['off', 'No limit'], [250, '250 MB'], [500, '500 MB'], [1000, '1 GB']]" />
      </SettingsRow>
      <SettingsRow title="A detail row with a stepper" detail>
        <Stepper v-model="demo.steps" :steps="[1, 2, 3, 5, 10, 20, 50]" />
      </SettingsRow>
      <SettingsRow as="button" title="A row that opens something" description="TROLLGANG">
        <template #end><ChevronRight class="size-[18px] text-foreground" /></template>
      </SettingsRow>
    </SettingsCard>

    <SettingsLabel mark="This device">A list: its icons share one column</SettingsLabel>
    <SettingsCard>
      <SettingsRow v-for="[id, name, sub] in DEVICES" :key="id" as="button" :title="name" :description="sub || undefined" @click="demo.device = id">
        <template #end><RadioMark :on="demo.device === id" /></template>
      </SettingsRow>
    </SettingsCard>

    <SettingsLabel>Tiles, dots, badges, a hint</SettingsLabel>
    <SettingsCard>
      <SettingsRow title="Music" description="1.012 songs · 2 only here · 105 already synced">
        <template #lead><IconTile :icon="Folder" /></template>
      </SettingsRow>
      <SettingsRow description="218 songs · your share, copied here">
        <template #lead><IconTile :icon="Cloud" /></template>
        <template #title>Synced music <SettingsBadge class="ml-1.5">Synced</SettingsBadge></template>
      </SettingsRow>
      <SettingsRow title="Y:\Family music" description="486 songs · streamed from \\nas\family">
        <template #lead><IconTile :icon="HardDrive" /></template>
      </SettingsRow>
      <SettingsRow title="Video Killed The Radio Star" description="The Buggles · the synced one plays" detail>
        <template #lead><span class="grid w-[34px] place-items-center"><Hint :icon="Copy" text="A copy: this song is also in your synced music, and Melue plays that one" /></span></template>
        <span class="text-[13px] text-muted-foreground">9,8 MB</span>
      </SettingsRow>
      <SettingsRow title="simon@nas/music · Synced" description="Synced 12 min ago · 3 copied, 1 moved">
        <template #lead><StatusDot kind="ok" /></template>
        <Button variant="outline" size="sm">Sync now</Button>
      </SettingsRow>
    </SettingsCard>

    <SettingsLabel>Fields, checkboxes, a question</SettingsLabel>
    <SettingsCard>
      <div class="settings-row grid gap-3 sm:grid-cols-2" data-row>
        <Field v-model="demo.address" label="A field" placeholder="\\nas\music or nas/music" hint="A hint under it" />
        <Field v-model="demo.address" label="A field with an error" error="Can’t reach “nas”. Check that it’s on." />
      </div>
      <SettingsRow title="Checkboxes" description="Picked, not picked, some picked (a folder’s “select all”)">
        <Checkbox :model-value="true" />
        <Checkbox :model-value="false" />
        <Checkbox model-value="indeterminate" />
      </SettingsRow>
      <SettingsRow title="A question before something big" description="Red when it can’t simply come back">
        <Button variant="outline" size="sm" @click="demo.asking = true">Ask</Button>
      </SettingsRow>
    </SettingsCard>
    <ConfirmDialog
      v-model:open="demo.asking"
      title="Remove the share?"
      text="Syncing stops on this device. The synced folder stays, with its songs."
      action="Remove share"
      danger
    />

    <SettingsLabel>Tips and buttons</SettingsLabel>
    <div class="grid gap-3">
      <SettingsTip>
        Tip: Look under Library to set up a share and folders first.
        <template #action><button type="button" class="text-[13px] font-medium underline underline-offset-3">Set one up</button></template>
      </SettingsTip>
      <SettingsTip kind="ok">Connected: 1,240 songs in nas/music</SettingsTip>
      <SettingsTip kind="bad">Couldn’t reach the share</SettingsTip>
      <div class="flex flex-wrap gap-2">
        <Button size="sm">Start sync</Button>
        <Button variant="outline" size="sm">Change folder</Button>
        <Button variant="danger" size="sm">Remove share</Button>
      </div>
    </div>
  </main>
</template>
