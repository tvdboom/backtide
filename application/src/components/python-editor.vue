<template>
  <div ref="editorElement" class="python-editor" />
</template>

<script setup>
import { python } from '@codemirror/lang-python'
import { indentWithTab } from '@codemirror/commands'
import { indentUnit } from '@codemirror/language'
import { Compartment, EditorState } from '@codemirror/state'
import { EditorView, keymap } from '@codemirror/view'
import { basicSetup } from 'codemirror'
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'

const props = defineProps({
  modelValue: { type: String, default: '' },
  readonly: { type: Boolean, default: false }
})
const emit = defineEmits(['update:modelValue'])
const editorElement = ref(null)
let view
let themeObserver
const themeCompartment = new Compartment()

defineExpose({ getView: () => view })

function backtideTheme() {
  const dark = document.documentElement.dataset.theme !== 'light'
  return EditorView.theme({
  '&': {
    color: 'var(--code-text)',
    backgroundColor: 'var(--code-bg)',
    fontSize: '11px'
  },
  '.cm-content': {
    caretColor: '#5ba1ff',
    fontFamily: "ui-monospace, 'Cascadia Code', monospace",
    lineHeight: '1.65',
    padding: '12px 0'
  },
  '.cm-cursor, .cm-dropCursor': { borderLeftColor: '#5ba1ff' },
  '.cm-gutters': {
    color: 'var(--code-gutter)',
    backgroundColor: 'var(--code-bg)',
    borderRight: '1px solid var(--line)'
  },
  '.cm-activeLine, .cm-activeLineGutter': { backgroundColor: 'rgba(38, 132, 255, .07)' },
  '.cm-selectionBackground, &.cm-focused .cm-selectionBackground': {
    backgroundColor: 'rgba(38, 132, 255, .25)'
  },
  '&.cm-focused': { outline: 'none' }
  }, { dark })
}

onMounted(() => {
  view = new EditorView({
    parent: editorElement.value,
    state: EditorState.create({
      doc: props.modelValue,
      extensions: [
        basicSetup,
        python(),
        EditorState.tabSize.of(4),
        indentUnit.of('    '),
        keymap.of([indentWithTab]),
        themeCompartment.of(backtideTheme()),
        EditorView.lineWrapping,
        EditorState.readOnly.of(props.readonly),
        EditorView.editable.of(!props.readonly),
        EditorView.updateListener.of(update => {
          if (update.docChanged) emit('update:modelValue', update.state.doc.toString())
        })
      ]
    })
  })
  themeObserver = new MutationObserver(() => {
    view?.dispatch({ effects: themeCompartment.reconfigure(backtideTheme()) })
  })
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })
})

watch(() => props.modelValue, value => {
  if (!view || value === view.state.doc.toString()) return
  view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: value } })
})

onBeforeUnmount(() => {
  themeObserver?.disconnect()
  view?.destroy()
})
</script>
