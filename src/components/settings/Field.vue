<script setup lang="ts">
/**
 * A labelled field in a form (the share's address, user, password): its label, the field, and under it the error
 * (which marks it) or a hint. For one value edited in a row, EditRow has Cancel and Save.
 */
import { useId } from 'vue'
import { Input } from '@/components/ui/input'

defineProps<{ label: string; type?: 'text' | 'password'; placeholder?: string; hint?: string; error?: string }>()
const model = defineModel<string>({ required: true })
const id = useId()
</script>

<template>
  <div class="grid min-w-0 content-start gap-1.5">
    <label :for="id" class="text-[13px] font-medium text-soft">{{ label }}</label>
    <Input
      :id="id"
      v-model="model"
      :type="type ?? 'text'"
      :placeholder="placeholder"
      :aria-invalid="!!error || undefined"
      :autocomplete="type === 'password' ? 'off' : undefined"
      spellcheck="false"
    />
    <small v-if="error" class="text-[12.5px] text-destructive">{{ error }}</small>
    <small v-else-if="hint" class="text-[12.5px] text-muted-foreground">{{ hint }}</small>
  </div>
</template>
