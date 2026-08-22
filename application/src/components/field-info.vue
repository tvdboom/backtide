<template>
  <span
    class="field-info"
    role="button"
    tabindex="0"
    :aria-label="`About this setting: ${text}`"
    @click.prevent.stop
    @keydown.space.prevent
  >
    <CircleHelp :size="14" aria-hidden="true" />
    <span class="field-info-popover" role="tooltip">{{ text }}</span>
  </span>
</template>

<script setup>
import { CircleHelp } from 'lucide-vue-next'

defineProps({
  text: { type: String, required: true }
})
</script>
