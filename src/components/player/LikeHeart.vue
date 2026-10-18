<script setup lang="ts">
/**
 * A like button's heart. Liked, it fills from the bottom up while it pops (grows and settles) and a burst of short
 * lines flies out of it; unliked, the fill drains away. It plays whenever the song becomes liked, wherever it is liked
 * from, so every heart of that song on screen (its row, the player bar, Now playing) does it at once. A heart that
 * comes into view already liked is just filled. Its size and colour come from the button around it (an svg's).
 */
import { Heart } from '@lucide/vue'
import { ref, watch } from 'vue'

const props = defineProps<{ liked: boolean }>()

// Each like: a new burst (its key restarts the lines' animation), and the pop.
const burst = ref(0)
const heart = ref<HTMLElement>()
watch(
  () => props.liked,
  (liked, was) => {
    if (!liked || was || matchMedia('(prefers-reduced-motion: reduce)').matches) return
    burst.value++
    heart.value?.animate([{ scale: 1 }, { scale: 0.85, offset: 0.15 }, { scale: 1.25, offset: 0.5 }, { scale: 1 }], {
      duration: 460,
      easing: 'cubic-bezier(.2,.8,.2,1)',
    })
  },
)
</script>

<template>
  <span ref="heart" class="like-heart" :class="liked && 'is-liked'">
    <Heart />
    <Heart class="like-heart-fill fill-current" />
    <span v-if="burst" :key="burst" class="like-heart-burst" aria-hidden="true">
      <i v-for="n in 8" :key="n" :style="{ '--turn': `${n * 45}deg` }" />
    </span>
  </span>
</template>
