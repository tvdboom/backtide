<template>
  <div class="search-select">
    <div class="tag-field" :class="{ focused }" @click="input?.focus()">
      <span v-for="item in modelValue" :key="item" class="tag">
        <img
          v-if="logos[item] && !failedLogos.has(item)"
          :src="logos[item]"
          alt=""
          @error="logoFailed(item)"
        />
        {{ item }}
        <button type="button" :aria-label="`Remove ${item}`" @click.stop="remove(item)">×</button>
      </span>
      <input
        :id="inputId"
        ref="input"
        v-model="needle"
        :placeholder="modelValue.length ? '' : placeholder"
        :aria-label="label"
        @focus="focused = true"
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
        @mousedown.prevent="choose(option)"
      >
        <span v-if="plainOptions" class="search-option-plain">{{ option }}</span>
        <template v-else>
          <span class="search-option-logo">
            <img
              v-if="logos[option] && !failedLogos.has(option)"
              :src="logos[option]"
              alt=""
              @error="logoFailed(option)"
            />
            <span v-else>{{ option.slice(0, 2) }}</span>
          </span>
          <span class="search-option-copy">
            <strong>{{ descriptions[option] || option }}</strong>
            <small>{{ option }}</small>
          </span>
        </template>
      </button>
    </div>
  </div>
</template>

<script setup>
import { computed, reactive, ref } from 'vue'

const props = defineProps({
  modelValue: { type: Array, default: () => [] },
  options: { type: Array, default: () => [] },
  descriptions: { type: Object, default: () => ({}) },
  logos: { type: Object, default: () => ({}) },
  placeholder: { type: String, default: 'Search…' },
  label: { type: String, default: 'Search and select' },
  allowCustom: Boolean,
  loading: Boolean,
  plainOptions: Boolean,
  inputId: { type: String, default: undefined },
  uppercaseCustom: { type: Boolean, default: true }
})
const emit = defineEmits(['update:modelValue'])
const input = ref(null)
const needle = ref('')
const focused = ref(false)
const failedLogos = reactive(new Set())
const filtered = computed(() => props.options
  .filter(item => !props.modelValue.includes(item))
  .filter(item => `${item} ${props.descriptions[item] || ''}`.toLowerCase().includes(needle.value.toLowerCase()))
  .slice(0, 10))

function logoFailed(value) {
  failedLogos.add(value)
}

function choose(value) {
  const clean = String(value || '').trim()
  const original = props.options.find(item => item.toUpperCase() === clean.toUpperCase())
  const custom = props.uppercaseCustom ? clean.toUpperCase() : clean
  const selected = original || (props.allowCustom ? custom : '')
  if (selected && !props.modelValue.includes(selected)) emit('update:modelValue', [...props.modelValue, selected])
  needle.value = ''
}
function remove(value) {
  emit('update:modelValue', props.modelValue.filter(item => item !== value))
}
function backspace() {
  if (!needle.value && props.modelValue.length) remove(props.modelValue.at(-1))
}
function close() {
  window.setTimeout(() => { focused.value = false }, 100)
}
</script>
