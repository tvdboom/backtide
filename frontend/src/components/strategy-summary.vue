<template>
  <span v-if="names.length" ref="summary" class="session-strategy-summary">
    <span ref="visible" class="session-strategy-visible">{{ visibleNames.join(', ') }}</span>
    <span
      v-if="hiddenNames.length"
      ref="trigger"
      class="session-strategy-overflow"
      role="button"
      tabindex="0"
      :aria-label="moreStrategiesLabel"
      :aria-describedby="tooltipId"
      @mouseenter="showFromHover"
      @mouseleave="hideFromHover"
      @focus="showFromFocus"
      @blur="hideFromFocus"
    >
      +{{ hiddenNames.length }}
    </span>
  </span>
  <span v-else>Monitor only</span>

  <Teleport to="body">
    <span
      v-if="tooltipVisible"
      :id="tooltipId"
      ref="tooltip"
      class="session-strategy-tooltip"
      :data-placement="tooltipPlacement"
      :style="tooltipStyle"
      role="tooltip"
    >
      <strong>More strategies</strong>
      <span>{{ hiddenNames.join(', ') }}</span>
    </span>
  </Teleport>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useId, watch } from 'vue'

const props = defineProps({
  names: { type: Array, default: () => [] }
})

const summary = ref(null)
const visible = ref(null)
const trigger = ref(null)
const tooltip = ref(null)
const hovered = ref(false)
const focused = ref(false)
const tooltipPlacement = ref('above')
const tooltipStyle = ref({})
const tooltipId = `strategy-summary-${useId()}`
const visibleCount = ref(Math.min(props.names.length, 2))
const visibleNames = computed(() => props.names.slice(0, visibleCount.value))
const hiddenNames = computed(() => props.names.slice(visibleCount.value))
const tooltipVisible = computed(() => hiddenNames.value.length && (hovered.value || focused.value))
const moreStrategiesLabel = computed(() => {
  const count = hiddenNames.value.length
  return `${count} more ${count === 1 ? 'strategy' : 'strategies'}`
})
let resizeObserver
let measurement = 0

async function fitVisibleNames() {
  const currentMeasurement = ++measurement
  visibleCount.value = Math.min(props.names.length, 2)
  await nextTick()
  if (currentMeasurement !== measurement || props.names.length < 2 || !visible.value) return
  if (visible.value.scrollWidth > visible.value.clientWidth + 1) visibleCount.value = 1
}

async function positionTooltip() {
  await nextTick()
  if (!trigger.value || !tooltip.value) return

  const triggerRect = trigger.value.getBoundingClientRect()
  const tooltipRect = tooltip.value.getBoundingClientRect()
  const tooltipWidth = tooltipRect.width || Math.min(280, window.innerWidth - 24)
  const tooltipHeight = tooltipRect.height || 72
  const viewportPadding = 12
  const centeredLeft = triggerRect.left + triggerRect.width / 2
  const minimumLeft = viewportPadding + tooltipWidth / 2
  const maximumLeft = window.innerWidth - viewportPadding - tooltipWidth / 2
  const placeBelow = triggerRect.top < tooltipHeight + viewportPadding

  tooltipPlacement.value = placeBelow ? 'below' : 'above'
  tooltipStyle.value = {
    left: `${Math.min(maximumLeft, Math.max(minimumLeft, centeredLeft))}px`,
    top: `${placeBelow ? triggerRect.bottom + 8 : triggerRect.top - 8}px`
  }
}

function showFromHover() {
  hovered.value = true
  positionTooltip()
}

function hideFromHover() {
  hovered.value = false
}

function showFromFocus() {
  focused.value = true
  positionTooltip()
}

function hideFromFocus() {
  focused.value = false
}

watch(() => props.names, fitVisibleNames, { deep: true })
onMounted(() => {
  fitVisibleNames()
  if (typeof ResizeObserver === 'function') {
    resizeObserver = new ResizeObserver(fitVisibleNames)
    resizeObserver.observe(summary.value)
  } else {
    addEventListener('resize', fitVisibleNames)
  }
})
onBeforeUnmount(() => {
  resizeObserver?.disconnect()
  removeEventListener('resize', fitVisibleNames)
})
</script>
