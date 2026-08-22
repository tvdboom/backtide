<template>
  <div ref="root" class="instrument-select">
    <button
      type="button"
      class="instrument-select-trigger"
      :aria-label="label"
      aria-haspopup="listbox"
      :aria-expanded="open"
      @click="open = !open"
    >
      <span class="search-option-logo">
        <img
          v-if="modelValue && logos[modelValue] && !failedLogos.has(modelValue)"
          :src="logos[modelValue]"
          alt=""
          @error="logoFailed(modelValue)"
        />
        <ChartCandlestick v-else :size="18" aria-hidden="true" />
      </span>
      <span class="search-option-copy">
        <strong>{{ modelValue || 'Select a symbol' }}</strong>
      </span>
      <ChevronDown :size="15" aria-hidden="true" />
    </button>
    <div v-if="open" class="search-menu instrument-select-menu" role="listbox" :aria-label="label">
      <button
        v-for="option in options"
        :key="option"
        type="button"
        role="option"
        :aria-selected="option === modelValue"
        @pointerdown.prevent
        @click.stop.prevent="choose(option)"
      >
        <span class="search-option-logo">
          <img
            v-if="logos[option] && !failedLogos.has(option)"
            :src="logos[option]"
            alt=""
            @error="logoFailed(option)"
          />
          <ChartCandlestick v-else :size="18" aria-hidden="true" />
        </span>
        <span class="search-option-copy">
          <strong>{{ option }}</strong>
          <small>{{ descriptions[option] }}</small>
        </span>
      </button>
    </div>
  </div>
</template>

<script setup>
import { ChartCandlestick, ChevronDown } from 'lucide-vue-next'
import { onBeforeUnmount, onMounted, reactive, ref } from 'vue'

const props = defineProps({
  modelValue: { type: String, default: '' },
  options: { type: Array, default: () => [] },
  descriptions: { type: Object, default: () => ({}) },
  logos: { type: Object, default: () => ({}) },
  label: { type: String, default: 'Select an instrument' }
})
const emit = defineEmits(['update:modelValue'])
const root = ref(null)
const open = ref(false)
const failedLogos = reactive(new Set())

function logoFailed(value) {
  failedLogos.add(value)
}

function choose(value) {
  emit('update:modelValue', value)
  open.value = false
}

function closeFromOutside(event) {
  if (!root.value?.contains(event.target)) open.value = false
}

onMounted(() => document.addEventListener('mousedown', closeFromOutside))
onBeforeUnmount(() => document.removeEventListener('mousedown', closeFromOutside))
</script>
