<script setup lang="ts">
/**
 * Slides its content open and closed (down and up) when `show` changes: a field that opens in a card (the address
 * at home). The space opens (a grid row from 0fr to 1fr) while the content drops into place and fades in, as one
 * motion, like switching pages in the playground (settings.css: reveal).
 */
// `line`: separated from the row above like a row (off for content with its own line, like a card's details).
withDefaults(defineProps<{ show: boolean; line?: boolean }>(), { line: true })
</script>

<template>
  <!-- 400 ms: the content's delay plus its drop (settings.css: --reveal-*); change them together. -->
  <Transition name="reveal" :duration="{ enter: 400, leave: 220 }">
    <div
      v-if="show"
      class="settings-reveal"
      :class="{ 'no-line': !line }"
    >
      <div class="min-h-0">
        <slot />
      </div>
    </div>
  </Transition>
</template>
