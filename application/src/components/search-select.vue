<template>
  <div class="search-select">
    <div class="tag-field" :class="{ focused }" @click="input?.focus()">
      <span
        v-for="item in modelValue"
        :key="item"
        class="tag"
        :class="{ detailed: showSelectedDescription, reorderable, dragging: draggedTag === item, 'drop-target': dragOverTag === item }"
        :draggable="reorderable"
        :title="reorderable ? `Drag ${item} to reorder` : undefined"
        @dragstart.stop="startTagDrag($event, item)"
        @dragover.stop.prevent="reorderTag($event, item)"
        @drop.stop.prevent="finishTagDrag"
        @dragend="finishTagDrag"
      >
        <span v-if="hasLogoEntry(item)" class="selected-symbol-logo" aria-hidden="true">
          <ChartCandlestick v-if="!loadedSelectedLogos.has(item)" :size="13" />
          <img
            v-if="selectedLogoSource(item)"
            :class="{ loaded: loadedSelectedLogos.has(item) }"
            :src="selectedLogoSource(item)"
            alt=""
            decoding="async"
            fetchpriority="high"
            loading="eager"
            @load="selectedLogoLoaded(item)"
            @error="selectedLogoFailed(item)"
          />
        </span>
        <span v-if="showSelectedDescription" class="tag-copy">
          <strong>{{ item }}</strong>
          <small v-if="descriptions[item]">{{ descriptions[item] }}</small>
        </span>
        <template v-else>{{ item }}</template>
        <button
          v-if="removable"
          type="button"
          :aria-label="`Remove ${item}`"
          @pointerdown.stop.prevent
          @click.stop="remove(item)"
        >×</button>
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
      />
    </div>
    <div v-if="focused && loading" class="search-menu search-state" role="status">
      <span class="spinner small" /> Loading options…
    </div>
    <div
      v-else-if="focused && filtered.length"
      class="search-menu"
      :class="{ 'instrument-option-menu': hasInstrumentOptions }"
    >
      <div :class="{ 'instrument-menu-options': hasInstrumentOptions }">
        <button
          v-for="(option, index) in filtered"
          :key="option"
          type="button"
          :class="{ 'plain-option': plainOptions, previewed: option === detailOption }"
          @pointerdown.prevent
          @click.stop.prevent="choose(option)"
          @mouseenter="showDetails(option)"
          @focus="showDetails(option)"
        >
          <span v-if="plainOptions" class="search-option-plain">
            <strong>{{ option }}</strong>
            <small v-if="descriptions[option]">{{ descriptions[option] }}</small>
          </span>
          <template v-else>
            <span class="search-option-logo">
              <img
                v-if="index < menuLogoRequestLimit && logos[option] && !failedMenuLogos.has(option)"
                :src="logos[option]"
                alt=""
                decoding="async"
                fetchpriority="low"
                loading="lazy"
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
      <InstrumentMenuDetails
        v-if="hasInstrumentOptions && detailOption"
        :details="optionDetails[detailOption] || {}"
        :display="display"
        :load-graph="Boolean(activeOption) && activeOption === detailOption"
        :logo="logos[detailOption] || ''"
        :symbol="detailOption"
      />
    </div>
    <div v-if="clearable" class="selector-clear-row">
      <button
        type="button"
        class="text-button selector-clear-button"
        :aria-label="`Clear all ${clearLabel}`"
        :disabled="modelValue.length === 0"
        @click="clearAll"
      ><X :size="13" /> Clear all</button>
    </div>
  </div>
</template>

<script setup>
import { ChartCandlestick, X } from 'lucide-vue-next'
import { computed, onBeforeUnmount, reactive, ref } from 'vue'
import InstrumentMenuDetails from './instrument-menu-details.vue'

const props = defineProps({
  modelValue: { type: Array, default: () => [] },
  options: { type: Array, default: () => [] },
  descriptions: { type: Object, default: () => ({}) },
  display: { type: Object, default: () => ({}) },
  logos: { type: Object, default: () => ({}) },
  selectedLogos: { type: Object, default: () => ({}) },
  placeholder: { type: String, default: 'Search…' },
  label: { type: String, default: 'Search and select' },
  allowCustom: Boolean,
  loading: Boolean,
  multiple: { type: Boolean, default: true },
  optionIcon: { type: [Object, Function], default: null },
  optionIcons: { type: Object, default: () => ({}) },
  optionDetails: { type: Object, default: () => ({}) },
  optionNameFirst: Boolean,
  plainOptions: Boolean,
  reorderable: Boolean,
  removable: { type: Boolean, default: true },
  resultLimit: { type: Number, default: 20 },
  clearable: Boolean,
  clearLabel: { type: String, default: 'selections' },
  showSelectedDescription: Boolean,
  inputId: { type: String, default: undefined },
  uppercaseCustom: { type: Boolean, default: true }
})
const emit = defineEmits(['update:modelValue'])
const input = ref(null)
const needle = ref('')
const focused = ref(false)
const draggedTag = ref('')
const dragOverTag = ref('')
const failedMenuLogos = reactive(new Set())
const loadedSelectedLogos = reactive(new Set())
const selectedLogoFailures = reactive(new Map())
const menuLogoRequestLimit = 12
const activeOption = ref('')
let closeTimer
const hasInstrumentOptions = computed(() => Object.keys(props.optionDetails).length > 0)
const filtered = computed(() => {
  const search = needle.value.trim().toLowerCase()
  return props.options
    .filter(item => !props.modelValue.includes(item))
    .filter(item => `${item} ${props.descriptions[item] || ''}`.toLowerCase().includes(search))
    .sort((left, right) => search
      ? matchScore(left, search) - matchScore(right, search) || left.localeCompare(right)
      : 0)
    .slice(0, props.resultLimit)
})
const detailOption = computed(() => filtered.value.includes(activeOption.value)
  ? activeOption.value
  : filtered.value[0] || '')

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
  const sources = [props.logos[value], props.selectedLogos[value]]
    .filter((source, index, sources) => source && sources.indexOf(source) === index)
  return [
    ...sources,
    ...sources.map(source => retryLogoSource(source, 1)),
    ...sources.map(source => retryLogoSource(source, 2))
  ]
}

function retryLogoSource(source, attempt) {
  const separator = source.includes('?') ? '&' : '?'
  return `${source}${separator}selected_retry=${attempt}`
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

function showDetails(option) {
  activeOption.value = option
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
  if (!focused.value) activeOption.value = ''
  focused.value = true
}
function remove(value) {
  emit('update:modelValue', props.modelValue.filter(item => item !== value))
}
function clearAll() {
  emit('update:modelValue', [])
  needle.value = ''
}
function startTagDrag(event, value) {
  if (!props.reorderable) {
    event.preventDefault()
    return
  }
  draggedTag.value = value
  dragOverTag.value = ''
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'move'
    event.dataTransfer.setData('text/plain', value)
  }
}
function reorderTag(event, targetValue) {
  const sourceValue = draggedTag.value
  if (!props.reorderable || !sourceValue || targetValue === sourceValue) return
  dragOverTag.value = targetValue
  const from = props.modelValue.indexOf(sourceValue)
  const target = props.modelValue.indexOf(targetValue)
  if (from < 0 || target < 0) return
  const bounds = event.currentTarget.getBoundingClientRect()
  let insertion = target + (event.clientX > bounds.left + bounds.width / 2 ? 1 : 0)
  if (from < insertion) insertion -= 1
  insertion = Math.max(0, Math.min(props.modelValue.length - 1, insertion))
  if (insertion === from) return
  const reordered = [...props.modelValue]
  const [value] = reordered.splice(from, 1)
  reordered.splice(insertion, 0, value)
  emit('update:modelValue', reordered)
}
function finishTagDrag() {
  draggedTag.value = ''
  dragOverTag.value = ''
}
function close() {
  window.clearTimeout(closeTimer)
  closeTimer = window.setTimeout(() => {
    focused.value = false
    activeOption.value = ''
  }, 100)
}
onBeforeUnmount(() => window.clearTimeout(closeTimer))
</script>
