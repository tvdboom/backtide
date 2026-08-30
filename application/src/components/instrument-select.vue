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
        @mouseenter="showPreview($event, option)"
        @mouseleave="hidePreview"
        @focus="showPreview($event, option)"
        @blur="hidePreview"
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
    <InstrumentPreview
      :anchor="previewAnchor"
      :details="optionDetails[previewOption] || {}"
      :logo="logos[previewOption] || ''"
      :symbol="previewOption"
      :visible="Boolean(previewOption)"
    />
  </div>
</template>

<script setup>
import { ChartCandlestick, ChevronDown } from 'lucide-vue-next'
import { onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import InstrumentPreview from './instrument-preview.vue'

const props = defineProps({
  modelValue: { type: String, default: '' },
  options: { type: Array, default: () => [] },
  descriptions: { type: Object, default: () => ({}) },
  logos: { type: Object, default: () => ({}) },
  optionDetails: { type: Object, default: () => ({}) },
  label: { type: String, default: 'Select an instrument' }
})
const emit = defineEmits(['update:modelValue'])
const root = ref(null)
const open = ref(false)
const failedLogos = reactive(new Set())
const previewOption = ref('')
const previewAnchor = ref(null)

function logoFailed(value) {
  failedLogos.add(value)
}

function choose(value) {
  emit('update:modelValue', value)
  open.value = false
  hidePreview()
}

function showPreview(event, option) {
  if (!props.optionDetails[option]) return
  previewOption.value = option
  previewAnchor.value = event.currentTarget
}

function hidePreview() {
  previewOption.value = ''
  previewAnchor.value = null
}

function closeFromOutside(event) {
  if (!root.value?.contains(event.target)) { open.value = false; hidePreview() }
}

onMounted(() => document.addEventListener('mousedown', closeFromOutside))
onBeforeUnmount(() => document.removeEventListener('mousedown', closeFromOutside))
</script>
