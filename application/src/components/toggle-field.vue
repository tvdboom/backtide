<template>
  <label class="toggle-label">
    <span class="toggle-title">{{ label }}</span>
    <FieldInfo :text="help" />
    <span class="toggle-control">
      <small class="toggle-description">{{ description }}</small>
      <input
        :id="inputId || undefined"
        :checked="modelValue"
        class="toggle"
        type="checkbox"
        @change="update"
      />
    </span>
  </label>
</template>

<script setup>
import FieldInfo from './field-info.vue'

defineProps({
  modelValue: { type: Boolean, default: false },
  label: { type: String, required: true },
  description: { type: String, required: true },
  help: { type: String, required: true },
  inputId: { type: String, default: '' }
})

const emit = defineEmits(['update:modelValue', 'change'])

function update(event) {
  emit('update:modelValue', event.target.checked)
  emit('change', event)
}
</script>
