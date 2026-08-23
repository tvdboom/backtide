<template>
  <span class="parameter-summary-container">
    <span class="parameter-summary">
      <span v-for="entry in entries.slice(0, 2)" :key="entry.name" class="parameter-value">{{ entry.label }}</span>
      <FieldInfo
        v-if="entries.length > 2"
        class="parameter-overflow"
        :trigger-text="`+${entries.length - 2}`"
        :text="entries.slice(2).map(entry => entry.label).join(' · ')"
        :label="`${entries.length - 2} more parameters: ${entries.slice(2).map(entry => entry.label).join(', ')}`"
      />
    </span>
    <span v-if="!entries.length" class="parameter-summary-empty">—</span>
  </span>
</template>

<script setup>
import { computed } from 'vue'
import FieldInfo from './field-info.vue'

const props = defineProps({
  parameters: { type: Object, default: () => ({}) }
})

const entries = computed(() => Object.entries(props.parameters).map(([name, value]) => ({
  name,
  label: `${name}=${formatParameter(value)}`
})))

function formatParameter(value) {
  return typeof value === 'number'
    ? value.toLocaleString(undefined, { maximumFractionDigits: 8 })
    : String(value ?? '—')
}
</script>
