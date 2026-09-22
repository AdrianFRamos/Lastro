'use strict'

const fixtureTags = [
  { id: 'fixture-a', label: 'Protocol fixture tag A', rfidHex: '8000130000000001', fixture: true },
  { id: 'fixture-b', label: 'Protocol fixture tag B', rfidHex: '8000130000000002', fixture: true },
]

const state = {
  tags: [...fixtureTags],
  latest: null,
  pollTimer: null,
}

const elements = {
  wireChip: document.querySelector('#wireChip'),
  stationChip: document.querySelector('#stationChip'),
  tagList: document.querySelector('#tagList'),
  tagForm: document.querySelector('#tagForm'),
  tagLabel: document.querySelector('#tagLabel'),
  tagHex: document.querySelector('#tagHex'),
  readerDropZone: document.querySelector('#readerDropZone'),
  readerInstruction: document.querySelector('#readerInstruction'),
  readerSubtext: document.querySelector('#readerSubtext'),
  readerResult: document.querySelector('#readerResult'),
  stationState: document.querySelector('#stationState'),
  stationStateDescription: document.querySelector('#stationStateDescription'),
  stationKey: document.querySelector('#stationKey'),
  stationId: document.querySelector('#stationId'),
  activeCapture: document.querySelector('#activeCapture'),
  captureAction: document.querySelector('#captureAction'),
  captureSequence: document.querySelector('#captureSequence'),
  captureAnimal: document.querySelector('#captureAnimal'),
  captureRevision: document.querySelector('#captureRevision'),
  captureId: document.querySelector('#captureId'),
  captureOldRfid: document.querySelector('#captureOldRfid'),
  eventLog: document.querySelector('#eventLog'),
  disconnectButton: document.querySelector('#disconnectButton'),
  resetButton: document.querySelector('#resetButton'),
}

function escapeHtml(value) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#039;')
}

function normalizeRfid(value) {
  return value.trim().toLowerCase().replace(/[\s:-]/g, '')
}

function shortHex(value, head = 10, tail = 8) {
  if (!value) return '—'
  if (value.length <= head + tail + 1) return value
  return `${value.slice(0, head)}…${value.slice(-tail)}`
}

function setChip(element, tone, text) {
  element.dataset.tone = tone
  element.querySelector('span').textContent = text
}

function setReaderResult(tone, title, detail) {
  elements.readerResult.dataset.tone = tone
  elements.readerResult.querySelector('strong').textContent = title
  elements.readerResult.querySelector('span:last-child').textContent = detail
}

function renderTags() {
  elements.tagList.innerHTML = state.tags
    .map(
      (tag) => `
        <article class="rfid-tag" draggable="true" data-tag-id="${escapeHtml(tag.id)}">
          <span class="tag-disc" aria-hidden="true"></span>
          <div class="tag-copy">
            <strong>${escapeHtml(tag.label)}</strong>
            <code>${escapeHtml(tag.rfidHex)}</code>
          </div>
          <button class="tag-read" type="button" data-read-tag="${escapeHtml(tag.id)}">Read</button>
        </article>
      `,
    )
    .join('')

  for (const tagElement of elements.tagList.querySelectorAll('.rfid-tag')) {
    tagElement.addEventListener('dragstart', (event) => {
      event.dataTransfer.effectAllowed = 'copy'
      event.dataTransfer.setData('text/plain', tagElement.dataset.tagId)
    })
  }

  for (const button of elements.tagList.querySelectorAll('[data-read-tag]')) {
    button.addEventListener('click', () => readTag(button.dataset.readTag))
  }
}

async function api(path, options = {}) {
  const response = await fetch(path, {
    ...options,
    headers: {
      'content-type': 'application/json',
      ...(options.headers || {}),
    },
  })
  const body = await response.json().catch(() => ({ error: `HTTP ${response.status}` }))
  if (!response.ok) throw new Error(body.error || `HTTP ${response.status}`)
  return body
}

async function readTag(tagId) {
  const tag = state.tags.find((candidate) => candidate.id === tagId)
  if (!tag) return

  setReaderResult('muted', `Reading ${tag.label}`, tag.rfidHex)
  try {
    const result = await api('/api/observe', {
      method: 'POST',
      body: JSON.stringify({ rfidHex: tag.rfidHex, label: tag.label }),
    })
    if (result.accepted) {
      setReaderResult(
        'success',
        `${tag.label} accepted`,
        'The Station built and signed EVENT_READY from this canonical RFID observation.',
      )
    } else if (result.reason === 'NO_ACTIVE_CAPTURE') {
      setReaderResult(
        'warning',
        `${tag.label} ignored`,
        'The Station was not in WAIT_RFID, so the observation was not consumed.',
      )
    } else {
      setReaderResult('danger', `${tag.label} rejected`, result.reason || 'Station rejected observation')
    }
    await refreshState()
  } catch (error) {
    setReaderResult('danger', 'Reader request failed', error.message)
  }
}

function renderState(snapshot) {
  state.latest = snapshot
  const { station, transport, activeCommand, faults } = snapshot

  setChip(
    elements.wireChip,
    transport.wireConnected ? 'success' : 'muted',
    transport.wireConnected ? 'Agent connected' : 'Agent disconnected',
  )

  const stationTone = station.state === 'WAIT_RFID' ? 'proof' : station.state === 'WAIT_ACK' ? 'warning' : 'muted'
  setChip(elements.stationChip, stationTone, `Station ${station.state.toLowerCase().replace('_', ' ')}`)

  elements.stationState.textContent = station.state
  elements.stationKey.textContent = station.publicKeyHex
  elements.stationKey.title = station.publicKeyHex
  elements.stationId.textContent = station.stationIdHex
  elements.stationId.title = station.stationIdHex

  if (station.state === 'WAIT_RFID') {
    elements.stationStateDescription.textContent = 'A valid COMMAND is active. One validated canonical RFID observation can now be consumed.'
    elements.readerInstruction.textContent = 'Bring one RFID tag into the simulated reader field'
    elements.readerSubtext.textContent = 'This observation will be consumed once for the active Lastro capture.'
    elements.readerDropZone.classList.add('is-ready')
  } else if (station.state === 'WAIT_ACK') {
    elements.stationStateDescription.textContent = 'EVENT_READY was signed and emitted. The Station is waiting for the matching Agent ACK.'
    elements.readerInstruction.textContent = 'Signed evidence is awaiting Agent acknowledgement'
    elements.readerSubtext.textContent = 'Additional RFID observations are ignored until the current capture completes.'
    elements.readerDropZone.classList.remove('is-ready')
  } else {
    elements.stationStateDescription.textContent = transport.wireConnected
      ? 'Agent wire is connected. Waiting for the Agent to dispatch a capture command.'
      : 'Waiting for an Agent wire connection and a capture command.'
    elements.readerInstruction.textContent = 'Waiting for a capture command'
    elements.readerSubtext.textContent = 'RFID observations outside WAIT_RFID are ignored by design.'
    elements.readerDropZone.classList.remove('is-ready')
  }

  elements.activeCapture.hidden = !activeCommand
  if (activeCommand) {
    elements.captureAction.textContent = activeCommand.action
    elements.captureSequence.textContent = `SEQ ${activeCommand.eventSequence}`
    elements.captureAnimal.textContent = shortHex(activeCommand.animalIdHex, 14, 10)
    elements.captureAnimal.title = activeCommand.animalIdHex
    elements.captureRevision.textContent = String(activeCommand.identityRevision)
    elements.captureId.textContent = shortHex(activeCommand.captureIdHex, 10, 8)
    elements.captureId.title = activeCommand.captureIdHex
    elements.captureOldRfid.textContent = activeCommand.expectedOldRfidHashHex === '0'.repeat(64)
      ? 'NONE / ORIGIN'
      : shortHex(activeCommand.expectedOldRfidHashHex, 12, 8)
    elements.captureOldRfid.title = activeCommand.expectedOldRfidHashHex
  }

  for (const button of document.querySelectorAll('[data-fault]')) {
    button.setAttribute('aria-pressed', faults[button.dataset.fault] ? 'true' : 'false')
  }

  renderLogs(snapshot.logs)
}

function renderLogs(logs) {
  if (!logs.length) {
    elements.eventLog.innerHTML = '<p class="empty-log">No simulator events yet.</p>'
    return
  }
  elements.eventLog.innerHTML = logs
    .map((entry) => {
      const date = new Date(entry.at)
      const time = Number.isNaN(date.valueOf()) ? entry.at : date.toISOString().slice(11, 23)
      return `
        <div class="log-row">
          <span class="log-time">${escapeHtml(time)}</span>
          <span class="log-kind">${escapeHtml(entry.kind)}</span>
          <span class="log-message">
            ${escapeHtml(entry.message)}
            ${entry.detail ? `<span class="log-detail">${escapeHtml(entry.detail)}</span>` : ''}
          </span>
        </div>
      `
    })
    .join('')
}

async function refreshState() {
  try {
    renderState(await api('/api/state'))
  } catch (error) {
    setChip(elements.wireChip, 'warning', 'Simulator unavailable')
    setChip(elements.stationChip, 'warning', 'State unavailable')
  }
}

function installDropZone() {
  const dropZone = elements.readerDropZone
  dropZone.addEventListener('dragover', (event) => {
    event.preventDefault()
    event.dataTransfer.dropEffect = 'copy'
    dropZone.classList.add('is-dragover')
  })
  dropZone.addEventListener('dragleave', () => dropZone.classList.remove('is-dragover'))
  dropZone.addEventListener('drop', (event) => {
    event.preventDefault()
    dropZone.classList.remove('is-dragover')
    readTag(event.dataTransfer.getData('text/plain'))
  })
}

function installTagForm() {
  elements.tagForm.addEventListener('submit', (event) => {
    event.preventDefault()
    const rfidHex = normalizeRfid(elements.tagHex.value)
    if (!/^[0-9a-f]{16}$/.test(rfidHex)) {
      setReaderResult('danger', 'Invalid simulated tag', 'Canonical RFID must be exactly 8 bytes / 16 hexadecimal characters.')
      elements.tagHex.focus()
      return
    }
    if (state.tags.some((tag) => tag.rfidHex === rfidHex)) {
      setReaderResult('warning', 'Tag already exists', 'Choose a different canonical RFID value for this simulated tag.')
      return
    }
    const label = elements.tagLabel.value.trim() || `Simulated tag ${state.tags.length + 1}`
    state.tags.push({ id: `custom-${Date.now()}`, label, rfidHex, fixture: false })
    renderTags()
    elements.tagForm.reset()
    setReaderResult('muted', `${label} added`, 'Drag the tag into the reader when a capture is in WAIT_RFID.')
  })
}

function installFaults() {
  for (const button of document.querySelectorAll('[data-fault]')) {
    button.addEventListener('click', async () => {
      const name = button.dataset.fault
      const enabled = button.getAttribute('aria-pressed') !== 'true'
      try {
        await api('/api/faults', { method: 'POST', body: JSON.stringify({ name, enabled }) })
        await refreshState()
      } catch (error) {
        setReaderResult('danger', 'Fault control failed', error.message)
      }
    })
  }

  elements.disconnectButton.addEventListener('click', async () => {
    try {
      await api('/api/disconnect', { method: 'POST', body: '{}' })
      await refreshState()
    } catch (error) {
      setReaderResult('danger', 'Disconnect failed', error.message)
    }
  })

  elements.resetButton.addEventListener('click', async () => {
    try {
      await api('/api/reset', { method: 'POST', body: '{}' })
      setReaderResult('muted', 'Simulator reset', 'Station is IDLE and all one-shot faults are cleared.')
      await refreshState()
    } catch (error) {
      setReaderResult('danger', 'Reset failed', error.message)
    }
  })
}

renderTags()
installDropZone()
installTagForm()
installFaults()
refreshState()
state.pollTimer = window.setInterval(refreshState, 750)
