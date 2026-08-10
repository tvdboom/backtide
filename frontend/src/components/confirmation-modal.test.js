// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import ConfirmationModal from './confirmation-modal.vue'

describe('confirmation modal', () => {
  it('describes the destructive action and waits for explicit confirmation', async () => {
    const wrapper = mount(ConfirmationModal, {
      props: {
        open: true,
        title: 'Delete Momentum study?',
        message: 'This action cannot be undone.'
      },
      attachTo: document.body
    })
    await wrapper.vm.$nextTick()

    const dialog = wrapper.get('[role="alertdialog"]')
    expect(dialog.attributes('aria-modal')).toBe('true')
    expect(dialog.text()).toContain('Delete Momentum study?')
    expect(dialog.text()).toContain('This action cannot be undone.')
    expect(document.activeElement).toBe(wrapper.get('.confirm-cancel').element)
    expect(wrapper.emitted('confirm')).toBeUndefined()

    await wrapper.get('.confirm-submit').trigger('click')

    expect(wrapper.emitted('confirm')).toHaveLength(1)
    wrapper.unmount()
  })

  it('can be canceled with Escape or the in-page backdrop', async () => {
    const wrapper = mount(ConfirmationModal, {
      props: { open: true, title: 'Delete item?', message: 'Are you sure?' }
    })
    await wrapper.vm.$nextTick()

    await wrapper.get('.confirm-cancel').trigger('keydown.escape')
    await wrapper.get('.modal-layer').trigger('mousedown')

    expect(wrapper.emitted('cancel')).toHaveLength(2)
  })
})
