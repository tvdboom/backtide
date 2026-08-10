<template>
  <div ref="root" class="currency-select">
    <button
      :id="inputId"
      ref="trigger"
      type="button"
      class="currency-trigger"
      aria-haspopup="listbox"
      :aria-expanded="open"
      :aria-label="`Base currency: ${selected.code}`"
      @click="toggle"
      @keydown.down.prevent="openMenu"
      @keydown.up.prevent="openMenu(true)"
      @keydown.esc.prevent="closeMenu"
    >
      <img
        v-if="flagUrl(selected)"
        class="currency-flag"
        :src="flagUrl(selected)"
        alt=""
        @error="markFlagFailed(selected)"
      />
      <span v-else class="currency-flag currency-flag-fallback" aria-hidden="true">{{ flagLabel(selected) }}</span>
      <strong>{{ selected.code }}</strong>
      <ChevronDown :size="14" aria-hidden="true" />
    </button>
    <div v-if="open" class="currency-menu" role="listbox" aria-label="Base currency">
      <button
        v-for="option in options"
        :key="option.code"
        type="button"
        role="option"
        :aria-selected="option.code === modelValue"
        @click="choose(option.code)"
        @keydown.down.prevent="focusSibling($event, 1)"
        @keydown.up.prevent="focusSibling($event, -1)"
        @keydown.esc.prevent="closeMenu"
      >
        <img
          v-if="flagUrl(option)"
          class="currency-flag"
          :src="flagUrl(option)"
          alt=""
          @error="markFlagFailed(option)"
        />
        <span v-else class="currency-flag currency-flag-fallback" aria-hidden="true">{{ flagLabel(option) }}</span>
        <strong>{{ option.code }}</strong>
        <small>{{ option.name }}</small>
        <Check v-if="option.code === modelValue" :size="14" aria-hidden="true" />
      </button>
    </div>
  </div>
</template>

<script setup>
import { Check, ChevronDown } from 'lucide-vue-next'
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'

const props = defineProps({
  modelValue: { type: String, required: true },
  options: { type: Array, default: () => [] },
  inputId: { type: String, default: undefined }
})
const emit = defineEmits(['update:modelValue'])
const root = ref(null)
const trigger = ref(null)
const open = ref(false)
const failedFlags = ref(new Set())
const selected = computed(() => props.options.find(option => option.code === props.modelValue) || {
  code: props.modelValue,
  country_code: '',
  flag: ''
})

function flagLabel(option) {
  return option.country_code?.toUpperCase() || option.flag || ''
}

function flagUrl(option) {
  const countryCode = String(option.country_code || '').toLowerCase()
  if (!/^[a-z]{2}$/.test(countryCode) || failedFlags.value.has(countryCode)) return ''
  return `https://flagcdn.com/${countryCode}.svg`
}

function markFlagFailed(option) {
  failedFlags.value = new Set(failedFlags.value).add(String(option.country_code).toLowerCase())
}

function toggle() {
  if (open.value) closeMenu()
  else openMenu()
}

async function openMenu(fromEnd = false) {
  open.value = true
  await nextTick()
  const items = [...(root.value?.querySelectorAll('[role="option"]') || [])]
  const selectedIndex = props.options.findIndex(option => option.code === props.modelValue)
  const index = fromEnd ? items.length - 1 : Math.max(selectedIndex, 0)
  items[index]?.focus()
}

function closeMenu({ restoreFocus = true } = {}) {
  open.value = false
  if (restoreFocus) trigger.value?.focus()
}

function choose(code) {
  emit('update:modelValue', code)
  closeMenu()
}

function focusSibling(event, offset) {
  const items = [...root.value.querySelectorAll('[role="option"]')]
  const index = items.indexOf(event.currentTarget)
  items[(index + offset + items.length) % items.length]?.focus()
}

function closeFromOutside(event) {
  if (!root.value?.contains(event.target)) closeMenu({ restoreFocus: false })
}

onMounted(() => document.addEventListener('mousedown', closeFromOutside))
onBeforeUnmount(() => document.removeEventListener('mousedown', closeFromOutside))
</script>
