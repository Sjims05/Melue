<script setup lang="ts">
/**
 * Connect device on a computer (the player bar's speaker button, Now playing's Connect device): your devices to play
 * on, this one first, the one playing ticked (player/remotePlayback.ts). The trigger is the slot. On a phone the same
 * list is a drawer (openDevicesDrawer).
 */
import { Check, LoaderCircle, Monitor, Smartphone, Unplug } from '@lucide/vue'
import { ref } from 'vue'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import type { Device } from '@/library/remote'
import { deviceNote, devicesToOffer, disconnect, disconnectPlayedFrom, playOn, playingOn } from '@/player/remotePlayback'

// `gap`: room between the trigger and the menu (px; the menu's own 4 by default).
defineProps<{ align?: 'start' | 'center' | 'end'; side?: 'top' | 'bottom'; gap?: number }>()

const devices = ref<Device[] | null>(null)
// Looked at each time it opens (who's online changes).
async function opened(open: boolean) {
  if (open) devices.value = await devicesToOffer()
}
const playing = (d: Device) => (d.this ? !playingOn.device : playingOn.device?.id === d.id)
</script>

<template>
  <DropdownMenu @update:open="opened">
    <DropdownMenuTrigger as-child><slot /></DropdownMenuTrigger>
    <DropdownMenuContent :align="align ?? 'end'" :side="side ?? 'top'" :side-offset="gap ?? 4" class="min-w-64">
      <DropdownMenuLabel class="text-xs font-medium text-muted-foreground">Play on</DropdownMenuLabel>
      <div v-if="!devices" class="flex items-center gap-2 px-2 py-1.5 text-sm text-muted-foreground"><LoaderCircle class="size-4 animate-spin" />Looking for your devices…</div>
      <DropdownMenuItem
        v-for="d in devices ?? []"
        :key="d.id"
        :disabled="!d.this && d.allow === 'off'"
        class="items-start gap-3 py-2"
        @select="playOn(d)"
      >
        <component :is="d.kind === 'phone' ? Smartphone : Monitor" class="mt-0.5" />
        <span class="flex min-w-0 flex-1 flex-col">
          <span class="truncate">{{ d.this ? `This device (${d.name})` : d.name }}</span>
          <span v-if="deviceNote(d)" class="text-xs text-muted-foreground">{{ deviceNote(d) }}</span>
        </span>
        <LoaderCircle v-if="playingOn.connecting === d.id" class="mt-0.5 animate-spin" />
        <Check v-else-if="playing(d)" class="mt-0.5" />
      </DropdownMenuItem>
      <template v-if="playingOn.device">
        <DropdownMenuSeparator />
        <DropdownMenuItem class="gap-3" @select="disconnect"><Unplug />Disconnect (keep playing on {{ playingOn.device.name }})</DropdownMenuItem>
      </template>
      <!-- The other way round: another device plays on this one. -->
      <template v-if="playingOn.playedFrom.length">
        <DropdownMenuSeparator />
        <DropdownMenuLabel class="text-xs font-medium text-muted-foreground">Played from {{ playingOn.playedFrom.join(' and ') }}</DropdownMenuLabel>
        <DropdownMenuItem class="gap-3" @select="disconnectPlayedFrom"><Unplug />Disconnect it (keep playing here)</DropdownMenuItem>
      </template>
      <div v-if="devices && devices.length < 2" class="max-w-64 px-2 py-1.5 text-xs text-muted-foreground">
        Your other devices show here when they run Melue with the same share.
      </div>
    </DropdownMenuContent>
  </DropdownMenu>
</template>
