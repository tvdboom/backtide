<template>
  <span class="execution-status">
    <span
      ref="trigger"
      class="badge"
      :class="statusClass"
      :tabindex="reason ? 0 : undefined"
      :aria-describedby="reason ? tooltipId : undefined"
      @mouseenter="hovered = true"
      @mouseleave="hovered = false"
      @focus="focused = true"
      @blur="focused = false"
    >
      {{ status }}
    </span>
  </span>

  <Teleport to="body">
    <span
      v-if="tooltipVisible"
      :id="tooltipId"
      ref="tooltip"
      class="execution-status-tooltip"
      :data-placement="tooltipPlacement"
      :style="tooltipStyle"
      role="tooltip"
    >
      {{ reason }}
    </span>
  </Teleport>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, ref, useId, watch } from 'vue'

const props = defineProps({
  status: { type: String, default: '' },
  reason: { type: String, default: '' }
})

const trigger = ref(null)
const tooltip = ref(null)
const hovered = ref(false)
const focused = ref(false)
const tooltipPlacement = ref('above')
const tooltipStyle = ref({})
const tooltipId = `execution-status-${useId()}`
const tooltipVisible = computed(() => props.reason && (hovered.value || focused.value))
const statusClass = computed(() => {
  const value = props.status.toLowerCase()
  if (value === 'filled') return 'success'
  if (value === 'rejected') return 'error'
  if (['canceled', 'pending'].includes(value)) return 'partial'
  return 'neutral'
})

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
