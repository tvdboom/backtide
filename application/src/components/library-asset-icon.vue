<template>
  <component :is="icon" :size="size" aria-hidden="true" />
</template>

<script setup>
import { Bot, Braces, Scale, Shapes, Sigma, SquareCode } from 'lucide-vue-next'
import { computed } from 'vue'

const props = defineProps({
  kind: { type: String, required: true },
  builtin: Boolean,
  size: { type: Number, default: 20 }
})

const icon = computed(() => {
  if (props.kind === 'strategy') return props.builtin ? Bot : SquareCode
  if (props.kind === 'metric') return props.builtin ? Sigma : Braces
  if (props.kind === 'sizer') return props.builtin ? Scale : Braces
  return props.builtin ? Shapes : Braces
})
</script>
