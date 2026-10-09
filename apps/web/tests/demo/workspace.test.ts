import { beforeEach, describe, expect, it } from 'vitest'
import { participants, resourcesOf, roles } from '../../src/demo/roles'
import {
  deleteRecord,
  listRecords,
  records,
  reloadWorkspace,
  resetRecords,
  saveRecord,
  session,
  signIn,
  signOut,
} from '../../src/demo/workspace'

const RECORDS_KEY = 'lastro.demo.records.v1'
const SESSION_KEY = 'lastro.demo.session.v1'

describe('demo workspace state', () => {
  beforeEach(() => {
    window.localStorage.clear()
    reloadWorkspace()
  })

  /**
   * ARRANGE: read the access profiles and the sections of each participant.
   * ACTION: list the section names per profile.
   * ASSERT: every profile has the sections asked for and only the common user is read-only.
   * FAILURE MEANS: a chain participant lost a section it must manage, or gained write access.
   */
  it('gives each chain participant its own sections and the common user read access', () => {
    const sections = Object.fromEntries(
      participants.map((role) => [role.id, resourcesOf(role.id).map((r) => r.label.pt)]),
    )

    expect(sections).toEqual({
      producer: [
        'Propriedades',
        'Lotes de animais',
        'Animais avulsos',
        'Antenas',
        'Vacinas',
        'Peso',
      ],
      carrier: ['Trajetos', 'Paradas', 'Características do transporte'],
      slaughterhouse: ['Peças', 'Perda'],
      exporter: ['Trajetos', 'Paradas', 'Características do transporte'],
      merchant: ['Peças', 'Venda', 'Perda'],
    })
    expect(roles.filter((role) => role.access === 'read').map((role) => role.id)).toEqual([
      'viewer',
    ])
  })

  /**
   * ARRANGE: start from the example rows.
   * ACTION: create, update and delete a property, then reload as a new page would.
   * ASSERT: each change is kept in this browser and numbers are stored as text.
   * FAILURE MEANS: the CRUD loses changes on reload or stores values the form cannot show.
   */
  it('creates, updates and deletes records and keeps them across a reload', () => {
    const before = listRecords('producer.properties').length

    const created = saveRecord('producer.properties', {
      name: ' Fazenda Nova ',
      city: 'Dourados/MS',
      area: 250.5,
      ignored: 'not a field',
    })
    expect(created).toMatchObject({ name: 'Fazenda Nova', city: 'Dourados/MS', area: '250.5' })
    expect(created).not.toHaveProperty('ignored')

    saveRecord('producer.properties', { ...created, area: '300' }, created.id)
    reloadWorkspace()
    expect(listRecords('producer.properties')).toHaveLength(before + 1)
    expect(listRecords('producer.properties').at(-1)?.area).toBe('300')

    deleteRecord('producer.properties', created.id)
    reloadWorkspace()
    expect(listRecords('producer.properties')).toHaveLength(before)
  })

  /**
   * ARRANGE: store malformed records in this browser.
   * ACTION: reload the workspace.
   * ASSERT: the example rows come back and the malformed value is removed.
   * FAILURE MEANS: a broken stored value can crash or corrupt the presentation.
   */
  it('fails closed to the example rows when stored records are malformed', () => {
    window.localStorage.setItem(RECORDS_KEY, JSON.stringify({ 'producer.lots': [{ id: 7 }] }))

    reloadWorkspace()

    expect(listRecords('producer.lots')[0]?.code).toBe('LT-2026-1012')
    expect(window.localStorage.getItem(RECORDS_KEY)).toBeNull()
  })

  /**
   * ARRANGE: change a record.
   * ACTION: restore the example data.
   * ASSERT: the change is gone, in memory and in storage.
   * FAILURE MEANS: a presenter cannot get back to a clean demo.
   */
  it('restores the example data', () => {
    saveRecord('merchant.losses', { weight: '1', reason: 'damaged', date: '2026-10-23' })
    expect(listRecords('merchant.losses')).toHaveLength(1)

    resetRecords()

    expect(records.value['merchant.losses']).toEqual([])
    expect(window.localStorage.getItem(RECORDS_KEY)).toBeNull()
  })

  /**
   * ARRANGE: no demo profile signed in.
   * ACTION: sign in as a carrier, reload, then sign out; then store an unknown profile.
   * ASSERT: the session survives a reload, is cleared on sign out and unknown profiles are dropped.
   * FAILURE MEANS: the workspace opens with the wrong permissions or none at all.
   */
  it('remembers the signed-in profile and rejects unknown ones', () => {
    signIn('carrier', 'carrier@lastro.demo')
    reloadWorkspace()
    expect(session.value).toEqual({ role: 'carrier', email: 'carrier@lastro.demo' })

    signOut()
    expect(session.value).toBeNull()

    window.localStorage.setItem(SESSION_KEY, JSON.stringify({ role: 'admin', email: 'x@y.z' }))
    reloadWorkspace()
    expect(session.value).toBeNull()
    expect(window.localStorage.getItem(SESSION_KEY)).toBeNull()
  })
})
