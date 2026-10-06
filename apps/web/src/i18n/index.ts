/**
 * Minimal two-language support (Brazilian Portuguese and English) for the public story and the
 * Open Demo. The first visit follows the browser language; a choice made with the flag switcher
 * is remembered in this browser. Components keep their own copy next to their markup and pick
 * the active language through `useCopy` or `tr`.
 */
import { computed, ref, type ComputedRef } from 'vue'

export type Locale = 'pt' | 'en'

/** One text in both languages. */
export interface Localized {
  pt: string
  en: string
}

const STORAGE_KEY = 'lastro.locale'

function isLocale(value: unknown): value is Locale {
  return value === 'pt' || value === 'en'
}

function initialLocale(): Locale {
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY)
    if (isLocale(stored)) return stored
  } catch {
    // Storage can be unavailable (private mode, sandboxed frames); fall back to the browser.
  }
  const language = typeof navigator === 'undefined' ? '' : navigator.language
  return language.toLowerCase().startsWith('pt') ? 'pt' : 'en'
}

function applyDocumentLanguage(next: Locale): void {
  if (typeof document !== 'undefined') {
    document.documentElement.lang = next === 'pt' ? 'pt-BR' : 'en'
  }
}

export const locale = ref<Locale>(initialLocale())
applyDocumentLanguage(locale.value)

export function setLocale(next: Locale): void {
  locale.value = next
  applyDocumentLanguage(next)
  try {
    window.localStorage.setItem(STORAGE_KEY, next)
  } catch {
    // The choice still applies for this visit.
  }
}

/** Text in the active language. */
export function tr(text: Localized): string {
  return text[locale.value]
}

/** A component's copy in the active language; both languages must have the same shape. */
export function useCopy<T>(copy: { en: T; pt: T }): ComputedRef<T> {
  return computed(() => copy[locale.value])
}

/** Plain text (same in both languages, such as a name or a hash) or a localized text. */
export type Text = string | Localized

export function tx(text: Text): string {
  return typeof text === 'string' ? text : tr(text)
}
