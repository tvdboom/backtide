<template>
  <div v-if="open" class="modal-layer" @mousedown.self="requestCancel">
    <section
      ref="dialog"
      class="modal panel confirm-modal"
      role="alertdialog"
      aria-modal="true"
      :aria-labelledby="titleId"
      :aria-describedby="descriptionId"
      @keydown.esc.stop.prevent="requestCancel"
      @keydown.tab="trapFocus"
    >
      <div class="confirm-heading">
        <span class="confirm-icon" aria-hidden="true"><Trash2 :size="21" /></span>
        <div>
          <span class="eyebrow">Confirm deletion</span>
          <h3 :id="titleId">{{ title }}</h3>
        </div>
        <button
          type="button"
          class="icon-button confirm-close"
          aria-label="Close confirmation"
          :disabled="busy"
          @click="requestCancel"
        ><X :size="17" /></button>
      </div>
      <p :id="descriptionId">{{ message }}</p>
      <div class="confirm-actions">
        <button ref="cancelButton" type="button" class="secondary confirm-cancel" :disabled="busy" @click="requestCancel">Cancel</button>
        <button type="button" class="confirm-submit" :disabled="busy" @click="requestConfirm">
          <span v-if="busy" class="spinner small" />
          <Trash2 v-else :size="15" />
          {{ busy ? 'Deleting...' : confirmLabel }}
        </button>
      </div>
    </section>
  </div>
</template>

<script setup>
import { Trash2, X } from 'lucide-vue-next'
import { nextTick, ref, useId, watch } from 'vue'

const props = defineProps({
  open: Boolean,
  title: { type: String, required: true },
  message: { type: String, required: true },
  confirmLabel: { type: String, default: 'Delete' },
  busy: Boolean
})
const emit = defineEmits(['cancel', 'confirm'])

const titleId = `confirmation-title-${useId()}`
const descriptionId = `confirmation-description-${useId()}`
const dialog = ref(null)
const cancelButton = ref(null)
let previousFocus = null

function requestCancel() {
  if (!props.busy) emit('cancel')
}

function requestConfirm() {
  if (!props.busy) emit('confirm')
}

function trapFocus(event) {
  const controls = [...dialog.value.querySelectorAll('button:not([disabled])')]
  const first = controls[0]
  const last = controls.at(-1)
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault()
    last.focus()
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault()
    first.focus()
  }
}

watch(() => props.open, async isOpen => {
  if (isOpen) {
    previousFocus = document.activeElement
    await nextTick()
    cancelButton.value?.focus()
  } else {
    previousFocus?.focus()
    previousFocus = null
  }
}, { immediate: true })
</script>
