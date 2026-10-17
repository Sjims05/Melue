<script setup lang="ts">
/**
 * − value +: steps through a list of numbers and saves as you tap, no typing (Ask first when it's more than: 5). The
 * number slides to the side as the next one comes in (settings.css: step-up / step-down).
 */
import { computed, ref } from 'vue'

const props = defineProps<{ steps: readonly number[] }>()
const model = defineModel<number>({ required: true })

const at = computed(() => Math.max(0, props.steps.indexOf(model.value)))
const direction = ref<'up' | 'down'>('up')
const step = (delta: number) => {
  direction.value = delta > 0 ? 'up' : 'down'
  model.value = props.steps[Math.min(props.steps.length - 1, Math.max(0, at.value + delta))]
}
</script>

<template>
  <span class="inline-flex items-center rounded-lg bg-secondary">
    <button type="button" class="grid size-8 place-items-center text-base disabled:opacity-40" :disabled="at === 0" aria-label="Less" @click="step(-1)">−</button>
    <!-- The old and the new number share one grid cell while they slide; the cell clips them. -->
    <span class="inline-grid h-8 min-w-7 place-items-center overflow-hidden" aria-live="polite">
      <Transition :name="`step-${direction}`">
        <b :key="model" class="col-start-1 row-start-1 text-sm font-semibold">{{ model }}</b>
      </Transition>
    </span>
    <button type="button" class="grid size-8 place-items-center text-base disabled:opacity-40" :disabled="at === steps.length - 1" aria-label="More" @click="step(1)">+</button>
  </span>
</template>
