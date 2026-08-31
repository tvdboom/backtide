<template>
  <div ref="root" class="instrument-select">
    <button
      type="button"
      class="instrument-select-trigger"
      :aria-label="label"
      aria-haspopup="listbox"
      :aria-expanded="open"
      @click="toggle"
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
    <div
      v-if="open"
      class="search-menu instrument-select-menu"
      :class="{ 'instrument-option-menu': hasInstrumentOptions }"
    >
      <div
        :class="{ 'instrument-menu-options': hasInstrumentOptions }"
        role="listbox"
        :aria-label="label"
      >
        <button
          v-for="option in options"
          :key="option"
          type="button"
          :class="{ previewed: option === detailOption }"
          role="option"
          :aria-selected="option === modelValue"
          @pointerdown.prevent
          @click.stop.prevent="choose(option)"
          @mouseenter="showDetails(option)"
          @focus="showDetails(option)"
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
      <InstrumentMenuDetails
        v-if="hasInstrumentOptions && detailOption"
        :details="optionDetails[detailOption] || {}"
        :display="display"
        :load-graph="Boolean(activeOption) && activeOption === detailOption"
        :logo="logos[detailOption] || ''"
        :symbol="detailOption"
      />
    </div>
  </div>
</template>

<script setup>
import { ChartCandlestick, ChevronDown } from 'lucide-vue-next'
import { computed, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import InstrumentMenuDetails from './instrument-menu-details.vue'

const props = defineProps({
  modelValue: { type: String, default: '' },
  options: { type: Array, default: () => [] },
  descriptions: { type: Object, default: () => ({}) },
  display: { type: Object, default: () => ({}) },
  logos: { type: Object, default: () => ({}) },
  optionDetails: { type: Object, default: () => ({}) },
  label: { type: String, default: 'Select an instrument' }
})
const emit = defineEmits(['update:modelValue'])
const root = ref(null)
const open = ref(false)
const failedLogos = reactive(new Set())
const activeOption = ref('')
const hasInstrumentOptions = computed(() => Object.keys(props.optionDetails).length > 0)
const detailOption = computed(() => props.options.includes(activeOption.value)
  ? activeOption.value
  : props.options[0] || '')

function logoFailed(value) {
  failedLogos.add(value)
}

function showDetails(option) {
  activeOption.value = option
}

function toggle() {
  open.value = !open.value
  activeOption.value = ''
}

function choose(value) {
  emit('update:modelValue', value)
  open.value = false
  activeOption.value = ''
}

function closeFromOutside(event) {
  if (!root.value?.contains(event.target)) {
    open.value = false
    activeOption.value = ''
  }
}

onMounted(() => document.addEventListener('mousedown', closeFromOutside))
onBeforeUnmount(() => document.removeEventListener('mousedown', closeFromOutside))
</script>
