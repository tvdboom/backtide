<template>
  <div ref="root" class="benchmark-select">
    <div class="benchmark-select-field" :class="{ focused }">
      <span v-if="modelValue" class="benchmark-select-logo">
        <img
          v-if="logos[modelValue] && !failedLogos.has(modelValue)"
          :src="logos[modelValue]"
          alt=""
          @error="logoFailed(modelValue)"
        />
        <Bot v-else-if="icon === 'strategy'" :size="17" aria-hidden="true" />
        <ChartCandlestick v-else :size="17" aria-hidden="true" />
      </span>
      <input
        ref="input"
        v-model="needle"
        :aria-label="label"
        :placeholder="placeholder"
        role="combobox"
        aria-autocomplete="list"
        :aria-expanded="focused"
        @focus="show"
        @input="focused = true"
        @keydown.enter.prevent="commit"
        @keydown.escape.prevent="close"
      />
      <button v-if="modelValue" type="button" :aria-label="`Clear ${modelValue} ${selectionName}`" @click="clear"><X :size="14" /></button>
      <ChevronDown v-else :size="15" aria-hidden="true" />
    </div>
    <div
      v-if="focused && filtered.length"
      class="search-menu benchmark-select-menu"
      :class="{ 'instrument-option-menu': hasInstrumentOptions }"
    >
      <div
        :class="{ 'instrument-menu-options': hasInstrumentOptions }"
        role="listbox"
        :aria-label="label"
      >
        <button
          v-for="option in filtered"
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
            <Bot v-else-if="icon === 'strategy'" :size="18" aria-hidden="true" />
            <ChartCandlestick v-else :size="18" aria-hidden="true" />
          </span>
          <span class="search-option-copy"><strong>{{ option }}</strong><small>{{ descriptions[option] }}</small></span>
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
import { Bot, ChartCandlestick, ChevronDown, X } from 'lucide-vue-next'
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import InstrumentMenuDetails from './instrument-menu-details.vue'

const props = defineProps({
  modelValue: { type: String, default: '' },
  options: { type: Array, default: () => [] },
  descriptions: { type: Object, default: () => ({}) },
  display: { type: Object, default: () => ({}) },
  logos: { type: Object, default: () => ({}) },
  optionDetails: { type: Object, default: () => ({}) },
  placeholder: { type: String, default: 'Search or enter a ticker…' },
  label: { type: String, default: 'Benchmark' },
  selectionName: { type: String, default: 'benchmark' },
  uppercaseValue: { type: Boolean, default: true },
  icon: { type: String, default: 'benchmark' }
})
const emit = defineEmits(['update:modelValue'])
const root = ref(null)
const input = ref(null)
const needle = ref(props.modelValue || '')
const focused = ref(false)
const failedLogos = reactive(new Set())
const activeOption = ref('')
const hasInstrumentOptions = computed(() => iconIsInstrument() &&
  Object.keys(props.optionDetails).length > 0)
const filtered = computed(() => {
  const search = needle.value === (props.modelValue || '') ? '' : needle.value.toLowerCase()
  return props.options
    .filter(option => option !== props.modelValue)
    .filter(option => `${option} ${props.descriptions[option] || ''}`.toLowerCase().includes(search))
    .sort((left, right) => search
      ? matchScore(left, search) - matchScore(right, search) || left.localeCompare(right)
      : 0)
    .slice(0, 20)
})
const detailOption = computed(() => filtered.value.includes(activeOption.value)
  ? activeOption.value
  : filtered.value[0] || '')

function matchScore(value, search) {
  if (!search) return 0
  const symbol = String(value).toLowerCase()
  const description = String(props.descriptions[value] || '').toLowerCase()
  if (symbol === search) return 0
  if (symbol.startsWith(search)) return 1
  if (description.startsWith(search)) return 2
  if (description.split(/[^a-z0-9]+/).some(word => word.startsWith(search))) return 3
  if (symbol.includes(search)) return 4
  return 5
}

function logoFailed(value) {
  failedLogos.add(value)
}

function iconIsInstrument() {
  return props.icon !== 'strategy'
}

function showDetails(option) {
  activeOption.value = option
}

function show() {
  if (!focused.value) activeOption.value = ''
  focused.value = true
  nextTick(() => input.value?.select())
}

function choose(value) {
  const normalized = String(value || '').trim()
  const selected = props.uppercaseValue ? normalized.toUpperCase() : normalized
  if (!selected) return
  emit('update:modelValue', selected)
  needle.value = selected
  focused.value = false
  activeOption.value = ''
}

function commit() {
  if (needle.value === props.modelValue) { close(); return }
  choose(filtered.value[0] || needle.value)
}

function clear() {
  emit('update:modelValue', null)
  needle.value = ''
  focused.value = false
  activeOption.value = ''
}

function close() {
  needle.value = props.modelValue || ''
  focused.value = false
  activeOption.value = ''
}

function closeFromOutside(event) {
  if (!root.value?.contains(event.target)) close()
}

watch(() => props.modelValue, value => { if (!focused.value) needle.value = value || '' })
onMounted(() => document.addEventListener('mousedown', closeFromOutside))
onBeforeUnmount(() => document.removeEventListener('mousedown', closeFromOutside))
</script>
