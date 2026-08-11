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
      <button v-if="modelValue" type="button" :aria-label="`Clear ${modelValue} benchmark`" @click="clear"><X :size="14" /></button>
      <ChevronDown v-else :size="15" aria-hidden="true" />
    </div>
    <div v-if="focused && filtered.length" class="search-menu benchmark-select-menu" role="listbox" :aria-label="label">
      <button
        v-for="option in filtered"
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
        <span class="search-option-copy"><strong>{{ option }}</strong><small>{{ descriptions[option] }}</small></span>
      </button>
    </div>
  </div>
</template>

<script setup>
import { ChartCandlestick, ChevronDown, X } from 'lucide-vue-next'
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'

const props = defineProps({
  modelValue: { type: String, default: '' },
  options: { type: Array, default: () => [] },
  descriptions: { type: Object, default: () => ({}) },
  logos: { type: Object, default: () => ({}) },
  placeholder: { type: String, default: 'Search or enter a ticker…' },
  label: { type: String, default: 'Benchmark' }
})
const emit = defineEmits(['update:modelValue'])
const root = ref(null)
const input = ref(null)
const needle = ref(props.modelValue || '')
const focused = ref(false)
const failedLogos = reactive(new Set())
const filtered = computed(() => {
  const search = needle.value === (props.modelValue || '') ? '' : needle.value.toLowerCase()
  return props.options
    .filter(option => option !== props.modelValue)
    .filter(option => `${option} ${props.descriptions[option] || ''}`.toLowerCase().includes(search))
    .slice(0, 10)
})

function logoFailed(value) {
  failedLogos.add(value)
}

function show() {
  focused.value = true
  nextTick(() => input.value?.select())
}

function choose(value) {
  const selected = String(value || '').trim().toUpperCase()
  if (!selected) return
  emit('update:modelValue', selected)
  needle.value = selected
  focused.value = false
}

function commit() {
  if (needle.value === props.modelValue) { close(); return }
  choose(filtered.value[0] || needle.value)
}

function clear() {
  emit('update:modelValue', null)
  needle.value = ''
  focused.value = false
}

function close() {
  needle.value = props.modelValue || ''
  focused.value = false
}

function closeFromOutside(event) {
  if (!root.value?.contains(event.target)) close()
}

watch(() => props.modelValue, value => { if (!focused.value) needle.value = value || '' })
onMounted(() => document.addEventListener('mousedown', closeFromOutside))
onBeforeUnmount(() => document.removeEventListener('mousedown', closeFromOutside))
</script>
