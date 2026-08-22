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
      @keydown="typeToSearch"
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
    <div v-if="open" class="currency-menu">
      <label class="currency-search">
        <Search :size="15" aria-hidden="true" />
        <input
          ref="searchInput"
          v-model="query"
          aria-label="Search base currencies"
          autocomplete="off"
          placeholder="Search currencies..."
          @keydown.down.prevent="focusOption(0)"
          @keydown.up.prevent="focusOption(filteredOptions.length - 1)"
          @keydown.enter.prevent="choose(filteredOptions[0]?.code)"
          @keydown.esc.prevent="closeMenu"
        />
      </label>
      <div class="currency-options" role="listbox" aria-label="Base currency">
        <button
          v-for="option in filteredOptions"
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
        <p v-if="!filteredOptions.length" class="currency-empty">No currencies match “{{ query.trim() }}”.</p>
      </div>
    </div>
  </div>
</template>

<script setup>
import { Check, ChevronDown, Search } from 'lucide-vue-next'
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'

const props = defineProps({
  modelValue: { type: String, required: true },
  options: { type: Array, default: () => [] },
  inputId: { type: String, default: undefined }
})
const emit = defineEmits(['update:modelValue'])
const root = ref(null)
const trigger = ref(null)
const searchInput = ref(null)
const open = ref(false)
const query = ref('')
const failedFlags = ref(new Set())
const selected = computed(() => props.options.find(option => option.code === props.modelValue) || {
  code: props.modelValue,
  country_code: '',
  flag: ''
})
const filteredOptions = computed(() => {
  const needle = query.value.trim().toLowerCase()
  if (!needle) return props.options
  return props.options.filter(option => `${option.code} ${option.name}`.toLowerCase().includes(needle))
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

async function openMenu(fromEnd = false, initialQuery = '') {
  open.value = true
  query.value = initialQuery
  await nextTick()
  if (fromEnd) focusOption(filteredOptions.value.length - 1)
  else searchInput.value?.focus()
}

function closeMenu({ restoreFocus = true } = {}) {
  open.value = false
  query.value = ''
  if (restoreFocus) trigger.value?.focus()
}

function choose(code) {
  if (!code) return
  emit('update:modelValue', code)
  closeMenu()
}

function focusOption(index) {
  const items = [...(root.value?.querySelectorAll('[role="option"]') || [])]
  items[index]?.focus()
}

function focusSibling(event, offset) {
  const items = [...root.value.querySelectorAll('[role="option"]')]
  const index = items.indexOf(event.currentTarget)
  items[(index + offset + items.length) % items.length]?.focus()
}

function typeToSearch(event) {
  if (event.key.length !== 1 || event.ctrlKey || event.altKey || event.metaKey) return
  event.preventDefault()
  openMenu(false, event.key)
}

function closeFromOutside(event) {
  if (!root.value?.contains(event.target)) closeMenu({ restoreFocus: false })
}

onMounted(() => document.addEventListener('mousedown', closeFromOutside))
onBeforeUnmount(() => document.removeEventListener('mousedown', closeFromOutside))
</script>
