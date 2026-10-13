import { describe, expect, it } from 'vitest'
import { useSystemFlow, type Scenario } from '../../src/system/flow'

async function runToEnd(scenario: Scenario) {
  const flow = useSystemFlow()
  flow.scenario.value = scenario
  while (!flow.finished.value) await flow.step()
  return flow
}

const states = (flow: ReturnType<typeof useSystemFlow>) =>
  Object.fromEntries(flow.steps.value.map((step) => [step.id, step.state]))

describe('system flow walkthrough', () => {
  /**
   * ARRANGE: a fresh walkthrough with the normal scenario.
   * ACTION: advance every step.
   * ASSERT: all seven steps succeed and the independent verifier reports valid.
   * FAILURE MEANS: the home page no longer demonstrates a correct capture end to end.
   */
  it('runs a normal capture from tag read to a valid independent verification', async () => {
    const flow = await runToEnd('normal')
    expect(Object.values(states(flow))).toEqual(Array(7).fill('done'))
    expect(flow.verdict.value).toBe('valid')
    const station = flow.steps.value.find((step) => step.id === 'station')!
    expect(station.details.find((detail) => detail.value === 'IDENTIFIER_BOUND')).toBeTruthy()
  }, 30_000)

  /**
   * ARRANGE: one envelope byte is flipped between the Agent and the API.
   * ACTION: advance every step.
   * ASSERT: the real P-256 check at the API rejects it and nothing later runs.
   * FAILURE MEANS: the demo would show a tampered capture being accepted.
   */
  it('stops a capture whose bytes were changed in transit at the API', async () => {
    const flow = await runToEnd('tampered')
    expect(states(flow)).toMatchObject({
      agent: 'done',
      api: 'failed',
      wallet: 'skipped',
      chain: 'skipped',
      verify: 'skipped',
    })
    expect(flow.verdict.value).toBe('rejected')
  }, 30_000)

  /**
   * ARRANGE: the capture comes from a Station that is not registered.
   * ACTION: advance every step.
   * ASSERT: the API rejects it although the signature itself is valid.
   * FAILURE MEANS: an unknown device could anchor evidence in the demo story.
   */
  it('rejects evidence from an unregistered Station', async () => {
    const flow = await runToEnd('unknownStation')
    expect(states(flow).api).toBe('failed')
    expect(flow.verdict.value).toBe('rejected')
  }, 30_000)

  /**
   * ARRANGE: the exported evidence package is edited after the event was anchored.
   * ACTION: advance every step.
   * ASSERT: everything up to Solana succeeds and only the independent verifier fails.
   * FAILURE MEANS: the demo no longer shows why verification must not trust the package.
   */
  it('flags a package edited after anchoring at the independent verifier', async () => {
    const flow = await runToEnd('alteredPackage')
    expect(states(flow)).toMatchObject({ chain: 'done', verify: 'failed' })
    expect(flow.verdict.value).toBe('invalid')
  }, 30_000)

  /**
   * ARRANGE: the farm has no connectivity for the first attempts.
   * ACTION: advance every step.
   * ASSERT: the Agent keeps the event queued, delivers it on a later attempt, and the flow ends valid.
   * FAILURE MEANS: the durable outbox behaviour is no longer shown.
   */
  it('keeps the event queued while offline and still finishes valid', async () => {
    const flow = await runToEnd('offline')
    const agent = flow.steps.value.find((step) => step.id === 'agent')!
    expect(agent.details.find((detail) => detail.value === '3')).toBeTruthy()
    expect(flow.log.value.filter((line) => line.tone === 'warn').length).toBeGreaterThanOrEqual(2)
    expect(flow.verdict.value).toBe('valid')
  }, 30_000)
})
