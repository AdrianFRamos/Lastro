import { afterEach } from 'vitest'

// Vue Test Utils mounts are owned by each test. Keep the global setup minimal:
// tests that mount a wrapper must unmount it when they install global side effects.
// Reset only DOM state shared through jsdom; do not import Testing Library cleanup().
if (typeof window !== 'undefined' && !window.matchMedia) {
  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: (query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: () => {},
      removeEventListener: () => {},
      dispatchEvent: () => false,
    }),
  })
}

afterEach(() => {
  document.body.innerHTML = ''
})
