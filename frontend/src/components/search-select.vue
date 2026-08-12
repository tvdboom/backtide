<template>
  <div class="search-select">
    <div class="tag-field" :class="{ focused }" @click="input?.focus()">
      <span v-for="item in modelValue" :key="item" class="tag" :class="{ detailed: showSelectedDescription }">
        <span v-if="hasLogoEntry(item)" class="selected-symbol-logo" aria-hidden="true">
          <ChartCandlestick v-if="!loadedSelectedLogos.has(item)" :size="13" />
          <img
            v-if="selectedLogoSource(item)"
            :class="{ loaded: loadedSelectedLogos.has(item) }"
            :src="selectedLogoSource(item)"
            alt=""
            @load="selectedLogoLoaded(item)"
            @error="selectedLogoFailed(item)"
          />
        </span>
        <span v-if="showSelectedDescription" class="tag-copy">
          <strong>{{ item }}</strong>
          <small v-if="descriptions[item]">{{ descriptions[item] }}</small>
        </span>
        <template v-else>{{ item }}</template>
        <button v-if="removable" type="button" :aria-label="`Remove ${item}`" @click.stop="remove(item)">×</button>
      </span>
      <input
        :id="inputId"
        ref="input"
        v-model="needle"
        :placeholder="modelValue.length ? '' : placeholder"
        :aria-label="label"
        @focus="open"
        @blur="close"
        @keydown.enter.prevent="choose(filtered[0] || needle)"
        @keydown.backspace="backspace"
      />
    </div>
    <div v-if="focused && loading" class="search-menu search-state" role="status">
      <span class="spinner small" /> Loading options…
    </div>
    <div v-else-if="focused && filtered.length" class="search-menu">
      <button
        v-for="option in filtered"
        :key="option"
        type="button"
        :class="{ 'plain-option': plainOptions }"
        @pointerdown.prevent
        @click.stop.prevent="choose(option)"
      >
        <span v-if="plainOptions" class="search-option-plain">
          <strong>{{ option }}</strong>
          <small v-if="descriptions[option]">{{ descriptions[option] }}</small>
        </span>
        <template v-else>
          <span class="search-option-logo">
            <img
              v-if="logos[option] && !failedMenuLogos.has(option)"
              :src="logos[option]"
              alt=""
              @error="logoFailed(option)"
            />
            <component :is="optionIconFor(option)" v-else-if="optionIconFor(option)" :size="18" aria-hidden="true" />
            <span v-else>{{ option.slice(0, 2) }}</span>
          </span>
          <span class="search-option-copy">
            <strong>{{ optionNameFirst ? option : descriptions[option] || option }}</strong>
            <small>{{ optionNameFirst ? descriptions[option] : option }}</small>
          </span>
        </template>
      </button>
    </div>
  </div>
</template>

<script setup>
import { ChartCandlestick } from 'lucide-vue-next'
import { computed, onBeforeUnmount, reactive, ref } from 'vue'

const props = defineProps({
  modelValue: { type: Array, default: () => [] },
  options: { type: Array, default: () => [] },
  descriptions: { type: Object, default: () => ({}) },
  logos: { type: Object, default: () => ({}) },
  selectedLogos: { type: Object, default: () => ({}) },
  placeholder: { type: String, default: 'Search…' },
  label: { type: String, default: 'Search and select' },
  allowCustom: Boolean,
  loading: Boolean,
  multiple: { type: Boolean, default: true },
  optionIcon: { type: [Object, Function], default: null },
  optionIcons: { type: Object, default: () => ({}) },
  optionNameFirst: Boolean,
  plainOptions: Boolean,
  removable: { type: Boolean, default: true },
  resultLimit: { type: Number, default: 100 },
  showSelectedDescription: Boolean,
  inputId: { type: String, default: undefined },
  uppercaseCustom: { type: Boolean, default: true }
})
const emit = defineEmits(['update:modelValue'])
const input = ref(null)
const needle = ref('')
const focused = ref(false)
const failedMenuLogos = reactive(new Set())
const loadedSelectedLogos = reactive(new Set())
const selectedLogoFailures = reactive(new Map())
let closeTimer
const filtered = computed(() => {
  const search = needle.value.trim().toLowerCase()
  return props.options
    .filter(item => !props.modelValue.includes(item))
    .filter(item => `${item} ${props.descriptions[item] || ''}`.toLowerCase().includes(search))
    .sort((left, right) => matchScore(left, search) - matchScore(right, search))
    .slice(0, props.resultLimit)
})

function logoFailed(value) {
  failedMenuLogos.add(value)
}

function hasLogoEntry(value) {
  return Object.prototype.hasOwnProperty.call(props.selectedLogos, value) ||
    Object.prototype.hasOwnProperty.call(props.logos, value)
}

function selectedLogoSource(value) {
  return selectedLogoCandidates(value)[selectedLogoFailures.get(value) || 0] || ''
}

function selectedLogoCandidates(value) {
  return [props.logos[value], props.selectedLogos[value]]
    .filter((source, index, sources) => source && sources.indexOf(source) === index)
}

function selectedLogoLoaded(value) {
  loadedSelectedLogos.add(value)
}

function selectedLogoFailed(value) {
  loadedSelectedLogos.delete(value)
  selectedLogoFailures.set(value, (selectedLogoFailures.get(value) || 0) + 1)
}

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

function optionIconFor(value) {
  return props.optionIcons[value] || props.optionIcon
}

function choose(value) {
  window.clearTimeout(closeTimer)
  const clean = String(value || '').trim()
  const original = props.options.find(item => item.toUpperCase() === clean.toUpperCase())
  const custom = props.uppercaseCustom ? clean.toUpperCase() : clean
  const selected = original || (props.allowCustom ? custom : '')
  if (selected && !props.modelValue.includes(selected)) {
    loadedSelectedLogos.delete(selected)
    selectedLogoFailures.delete(selected)
    emit('update:modelValue', props.multiple ? [...props.modelValue, selected] : [selected])
  }
  needle.value = ''
  if (!props.multiple) focused.value = false
}
function open() {
  window.clearTimeout(closeTimer)
  focused.value = true
}
function remove(value) {
  emit('update:modelValue', props.modelValue.filter(item => item !== value))
}
function backspace() {
  if (!needle.value && props.modelValue.length) remove(props.modelValue.at(-1))
}
function close() {
  window.clearTimeout(closeTimer)
  closeTimer = window.setTimeout(() => { focused.value = false }, 100)
}
onBeforeUnmount(() => window.clearTimeout(closeTimer))
</script>
