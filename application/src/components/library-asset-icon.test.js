// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import LibraryAssetIcon from './library-asset-icon.vue'

describe('library asset icon', () => {
  it('uses stable, distinct icons for built-in and custom assets', () => {
    const variants = [
      ['strategy', true, 'lucide-bot-icon'],
      ['strategy', false, 'lucide-square-code-icon'],
      ['indicator', true, 'lucide-shapes-icon'],
      ['indicator', false, 'lucide-braces-icon']
    ]

    for (const [kind, builtin, className] of variants) {
      const wrapper = mount(LibraryAssetIcon, { props: { kind, builtin } })
      expect(wrapper.get('svg').classes()).toContain(className)
      wrapper.unmount()
    }
  })
})
