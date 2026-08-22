// @vitest-environment jsdom
import { indentUnit } from '@codemirror/language'
import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { describe, expect, it } from 'vitest'
import PythonEditor from './python-editor.vue'

describe('python editor', () => {
  it('keeps Tab in the editor and indents Python with four spaces', async () => {
    const wrapper = mount(PythonEditor, {
      attachTo: document.body,
      props: { modelValue: 'pass' }
    })
    await nextTick()

    const view = wrapper.vm.getView()
    view.dispatch({ selection: { anchor: 0 } })
    view.contentDOM.focus()
    const tabEvent = new KeyboardEvent('keydown', {
      key: 'Tab',
      code: 'Tab',
      bubbles: true,
      cancelable: true
    })
    view.contentDOM.dispatchEvent(tabEvent)
    await nextTick()

    expect(tabEvent.defaultPrevented).toBe(true)
    expect(document.activeElement).toBe(view.contentDOM)
    expect(view.state.doc.toString()).toBe('    pass')
    expect(view.state.facet(indentUnit)).toBe('    ')
    expect(view.state.tabSize).toBe(4)

    wrapper.unmount()
  })
})
