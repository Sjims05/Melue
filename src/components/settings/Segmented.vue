<script setup lang="ts" generic="T extends string | number">
/**
 * One of a few choices as segments (Volume curve: Quadratic / Cubic / Linear). Always one is picked.
 *
 * The picked one's background slides to it and inverts what it covers: it carries a copy of the labels in the picked
 * colour, held in place as it moves (it moves one way, the copy the other way, at the same pace), so the inverted
 * text is revealed under it even halfway across a label.
 */
import { ToggleGroupItem, ToggleGroupRoot } from 'reka-ui'
import { nextTick, onActivated, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'

defineProps<{ options: readonly (readonly [T, string])[] }>()
const model = defineModel<T>({ required: true })

// Clicking the picked one again would unpick it: keep it picked.
const pick = (value: unknown) => {
  if (value !== undefined && value !== '') model.value = value as T
}

const root = ref<{ $el: HTMLElement }>()
// Where the picked segment is, and how wide the whole group is (the labels' copy is laid out like the group).
// `moves` stays off until it's placed the first time, so nothing slides in from the corner when the page opens.
const pill = reactive({ x: 0, y: 0, width: 0, height: 0, groupWidth: 0, moves: false })
// Measured exactly (fractions of a pixel): the labels' copy must be laid out like the group, and rounded down by a
// fraction, its labels wrap onto a second line (out of sight under the pill).
function place() {
  const group = root.value?.$el
  const picked = group?.querySelector<HTMLElement>('[data-state="on"]')
  if (!group || !picked) return
  const g = group.getBoundingClientRect()
  const p = picked.getBoundingClientRect()
  // Hidden (a kept-alive page that isn't shown): no sizes to go by; measuring now would make it grow back when shown.
  if (!g.width || !p.width) return
  Object.assign(pill, { x: p.left - g.left, y: p.top - g.top, width: p.width, height: p.height, groupWidth: g.width })
}

let resizes: ResizeObserver | undefined
onMounted(() => {
  place()
  requestAnimationFrame(() => (pill.moves = true))
  resizes = new ResizeObserver(place)
  resizes.observe(root.value!.$el)
})
onBeforeUnmount(() => resizes?.disconnect())
// Shown again (a kept-alive page): placed where it is, without sliding there.
onActivated(() => {
  pill.moves = false
  place()
  requestAnimationFrame(() => (pill.moves = true))
})
watch(model, () => nextTick(place))

const LABEL = 'h-[30px] rounded-[7px] px-3 text-[13px] font-medium whitespace-nowrap'
const MOVES = 'transition-transform duration-200 ease-out motion-reduce:transition-none'
</script>

<template>
  <ToggleGroupRoot
    ref="root"
    type="single"
    :model-value="model"
    class="relative inline-flex flex-wrap rounded-[9px] bg-well shadow-[inset_0_0_0_1px_var(--well-line)] p-[3px]"
    @update:model-value="pick"
  >
    <ToggleGroupItem
      v-for="[value, label] in options"
      :key="value"
      :value="value"
      :class="[LABEL, 'text-muted-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring/50']"
    >
      {{ label }}
    </ToggleGroupItem>
    <!-- The sliding background, over the labels (clicks go through to them), with the labels' inverted copy. -->
    <span
      aria-hidden="true"
      class="pointer-events-none absolute top-0 left-0 overflow-hidden rounded-[7px] bg-primary"
      :class="pill.moves && 'transition-[transform,width,height] duration-200 ease-out motion-reduce:transition-none'"
      :style="{ transform: `translate(${pill.x}px, ${pill.y}px)`, width: `${pill.width}px`, height: `${pill.height}px` }"
    >
      <span
        class="absolute top-0 left-0 flex flex-wrap p-[3px] text-primary-foreground"
        :class="pill.moves && MOVES"
        :style="{ transform: `translate(${-pill.x}px, ${-pill.y}px)`, width: `${pill.groupWidth}px` }"
      >
        <span v-for="[value, label] in options" :key="value" :class="[LABEL, 'inline-grid place-items-center']">{{ label }}</span>
      </span>
    </span>
  </ToggleGroupRoot>
</template>
