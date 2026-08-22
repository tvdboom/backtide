<template>
  <div
    :id="inputId"
    class="interval-picker"
    :role="multiple ? 'group' : 'radiogroup'"
    :aria-label="label"
  >
    <button
      v-for="option in options"
      :key="option"
      type="button"
      :role="multiple ? undefined : 'radio'"
      :aria-checked="multiple ? undefined : isSelected(option)"
      :aria-pressed="multiple ? isSelected(option) : undefined"
      :class="{ selected: isSelected(option) }"
      :disabled="isDisabled(option)"
      @click="choose(option)"
    >
      {{ option }}
    </button>
  </div>
</template>

<script setup>
const props = defineProps({
  modelValue: { type: [String, Array], default: '' },
  options: { type: Array, default: () => [] },
  values: { type: Object, default: () => ({}) },
  disabledOptions: { type: Array, default: () => [] },
  multiple: Boolean,
  label: { type: String, default: 'Interval' },
  inputId: { type: String, default: undefined }
})
const emit = defineEmits(['update:modelValue'])

function optionValue(option) {
  return Object.prototype.hasOwnProperty.call(props.values, option)
    ? props.values[option]
    : option
}

function selectedValues() {
  if (Array.isArray(props.modelValue)) return props.modelValue
  return props.modelValue ? [props.modelValue] : []
}

function isSelected(option) {
  return selectedValues().includes(optionValue(option))
}

function isDisabled(option) {
  return props.disabledOptions.includes(option)
    || props.disabledOptions.includes(optionValue(option))
}

function choose(option) {
  if (isDisabled(option)) return
  const value = optionValue(option)
  if (!props.multiple) {
    if (props.modelValue !== value) emit('update:modelValue', value)
    return
  }

  const selected = selectedValues()
  if (selected.includes(value)) {
    if (selected.length > 1) {
      emit('update:modelValue', selected.filter(item => item !== value))
    }
    return
  }
  emit('update:modelValue', [...selected, value])
}
</script>
