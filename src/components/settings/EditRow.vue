<script setup lang="ts">
/**
 * A row being edited: its title and grey line, a field, Cancel and Save (Enter saves, Escape cancels). The device's
 * name, the share's password, its address at home. `error` shows under the field (and marks it), `hint` otherwise.
 * Without a title and description it's just the field, under the row that opened it.
 */
import { onMounted, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'

const props = withDefaults(
  defineProps<{
    title?: string
    description?: string
    value?: string
    type?: 'text' | 'password'
    placeholder?: string
    hint?: string
    error?: string
    saving?: boolean
  }>(),
  { value: '', type: 'text' },
)
const emit = defineEmits<{ save: [value: string]; cancel: [] }>()

const draft = ref(props.value)
const field = ref<{ $el: HTMLInputElement }>()
// Ready to type, without scrolling: in a Reveal that's still opening, scrolling the field into view would make it
// travel with the opening.
onMounted(() => field.value?.$el.focus({ preventScroll: true }))
</script>

<template>
  <div class="settings-row flex flex-col items-stretch gap-3" data-row>
    <span v-if="title || description" class="flex flex-col gap-0.5">
      <span v-if="title" class="text-sm font-semibold">{{ title }}</span>
      <span v-if="description" class="text-[13.5px] leading-snug text-muted-foreground">{{ description }}</span>
    </span>
    <form class="flex items-center gap-2" @submit.prevent="emit('save', draft)">
      <Input
        ref="field"
        v-model="draft"
        :type="type"
        :placeholder="placeholder"
        :aria-invalid="!!error || undefined"
        :autocomplete="type === 'password' ? 'off' : undefined"
        class="flex-1"
        @keydown.esc="emit('cancel')"
      />
      <Button type="button" variant="outline" size="sm" @click="emit('cancel')">Cancel</Button>
      <Button type="submit" size="sm" :disabled="saving">Save</Button>
    </form>
    <small v-if="error" class="text-[12.5px] text-destructive">{{ error }}</small>
    <small v-else-if="hint" class="text-[12.5px] text-muted-foreground">{{ hint }}</small>
  </div>
</template>
