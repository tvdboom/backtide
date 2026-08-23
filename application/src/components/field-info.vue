<template>
  <span
    ref="trigger"
    class="field-info"
    :class="{ 'text-trigger': triggerText }"
    role="button"
    tabindex="0"
    :aria-label="label || `About this setting: ${text}`"
    :aria-describedby="tooltipId"
    @click.prevent.stop
    @keydown.space.prevent
    @mouseenter="hovered = true"
    @mouseleave="hovered = false"
    @focus="focused = true"
    @blur="focused = false"
  >
    <span v-if="triggerText">{{ triggerText }}</span>
    <CircleHelp v-else :size="14" aria-hidden="true" />

    <Teleport to="body">
      <span
        v-if="tooltipVisible"
        :id="tooltipId"
        ref="tooltip"
        class="field-info-popover"
        :data-placement="tooltipPlacement"
        :style="tooltipStyle"
        role="tooltip"
      >
        {{ text }}
      </span>
    </Teleport>
  </span>
</template>

<script setup>
import { CircleHelp } from 'lucide-vue-next'
import { computed, nextTick, onBeforeUnmount, ref, useId, watch } from 'vue'

defineProps({
  text: { type: String, required: true },
  triggerText: { type: String, default: '' },
  label: { type: String, default: '' }
})

const trigger = ref(null)
const tooltip = ref(null)
const hovered = ref(false)
const focused = ref(false)
const tooltipPlacement = ref('above')
const tooltipStyle = ref({})
const tooltipId = `field-info-${useId()}`
const tooltipVisible = computed(() => hovered.value || focused.value)

async function positionTooltip() {
  await nextTick()
  if (!trigger.value || !tooltip.value) return

  const triggerRect = trigger.value.getBoundingClientRect()
  const tooltipRect = tooltip.value.getBoundingClientRect()
  const viewportPadding = 12
  const tooltipGap = 8
  const tooltipWidth = tooltipRect.width || Math.min(280, window.innerWidth - 24)
  const tooltipHeight = tooltipRect.height || 50
  const centeredLeft = triggerRect.left + triggerRect.width / 2
  const minimumLeft = viewportPadding + tooltipWidth / 2
  const maximumLeft = window.innerWidth - viewportPadding - tooltipWidth / 2
  const spaceAbove = triggerRect.top - viewportPadding - tooltipGap
  const spaceBelow = window.innerHeight - triggerRect.bottom - viewportPadding - tooltipGap
  const placeBelow = spaceAbove < tooltipHeight && spaceBelow > spaceAbove

  tooltipPlacement.value = placeBelow ? 'below' : 'above'
  tooltipStyle.value = {
    left: `${Math.min(maximumLeft, Math.max(minimumLeft, centeredLeft))}px`,
    top: `${placeBelow ? triggerRect.bottom + tooltipGap : triggerRect.top - tooltipGap}px`
  }
}

function stopPositioning() {
  removeEventListener('resize', positionTooltip)
  removeEventListener('scroll', positionTooltip, true)
}

watch(tooltipVisible, visible => {
  stopPositioning()
  if (!visible) return
  positionTooltip()
  addEventListener('resize', positionTooltip)
  addEventListener('scroll', positionTooltip, true)
})

onBeforeUnmount(stopPositioning)
</script>
