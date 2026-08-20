import assert from 'node:assert/strict'
import { chmod, mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { test } from 'node:test'
import { apply } from './index.js'

async function fakeBin() {
  const dir = await mkdtemp(join(tmpdir(), 'rustiq-bin-'))
  const bin = join(dir, 'rustiq')
  await writeFile(bin, '#!/bin/sh\nexit 0\n')
  await chmod(bin, 0o755)
  return bin
}

function mockCtx() {
  const tools = []
  const sections = []
  const sends = []
  const kills = []
  const reads = []
  const agent = { session: { header: { cwd: '/repo' } } }
  return {
    sections,
    tools,
    sends,
    kills,
    reads,
    agent,
    ctx: {
      systemPrompt: { section: (s) => sections.push(s) },
      tools: { register: (def) => tools.push(def) },
      terminals: {
        spawn: async (_owner, spec) => ({ sessionId: 'sess-1', ...spec }),
        startSend: (_owner, id, req) => {
          sends.push({ id, req })
          return {
            done: Promise.resolve({ viewport: 'RUSTIQ SCREEN', waitReason: 'inferred_idle' }),
          }
        },
        read: (_owner, id, opts) => {
          reads.push({ id, opts })
          return { text: 'screen', totalLines: 2, truncated: false }
        },
        kill: async (_owner, id) => {
          kills.push(id)
          return true
        },
      },
    },
  }
}

test('apply registers four tools and guidance', () => {
  const m = mockCtx()
  apply(m.ctx)
  assert.equal(m.sections[0].name, 'tool:rustiq')
  assert.deepEqual(m.tools.map((t) => t.name), [
    'rustiq_open',
    'rustiq_send',
    'rustiq_read',
    'rustiq_close',
  ])
})

test('apply rejects negative maxResultBytes', () => {
  const m = mockCtx()
  assert.throws(() => apply(m.ctx, { maxResultBytes: -1 }), /non-negative integer/)
})

test('rustiq_send and close require agent and sessionId', async () => {
  const m = mockCtx()
  apply(m.ctx)
  const send = m.tools.find((t) => t.name === 'rustiq_send')
  const close = m.tools.find((t) => t.name === 'rustiq_close')
  const signal = AbortSignal.timeout(5_000)
  await assert.rejects(() => send.execute({ sessionId: 'x', text: 'j' }, { signal }), /initiating agent/)
  await assert.rejects(() => send.execute({ sessionId: '', text: 'j' }, { signal, agent: m.agent }), /sessionId/)
  const closed = await close.execute({ sessionId: 'sess-1' }, { signal, agent: m.agent })
  assert.equal(closed.outcome, 'closed')
  assert.deepEqual(m.kills, ['sess-1'])
})

test('rustiq_read pages the PTY', async () => {
  const m = mockCtx()
  apply(m.ctx)
  const read = m.tools.find((t) => t.name === 'rustiq_read')
  const out = await read.execute(
    { sessionId: 'sess-1', count: 20 },
    { signal: AbortSignal.timeout(5_000), agent: m.agent },
  )
  assert.equal(out.text, 'screen')
  assert.equal(m.reads[0].id, 'sess-1')
})

test('rustiq_send defaults submit to false', async () => {
  const m = mockCtx()
  apply(m.ctx)
  const send = m.tools.find((t) => t.name === 'rustiq_send')
  await send.execute(
    { sessionId: 'sess-1', text: 'j' },
    { signal: AbortSignal.timeout(5_000), agent: m.agent },
  )
  assert.equal(m.sends[0].req.submit, false)
})

test('rustiq_open kills the PTY if launch send fails', async () => {
  const bin = await fakeBin()
  const m = mockCtx()
  m.ctx.terminals.startSend = () => ({ done: Promise.reject(new Error('send failed')) })
  apply(m.ctx, { binaryPath: bin })
  const open = m.tools.find((t) => t.name === 'rustiq_open')
  await assert.rejects(
    () => open.execute({}, { signal: AbortSignal.timeout(5_000), agent: m.agent }),
    /send failed/,
  )
  assert.deepEqual(m.kills, ['sess-1'])
})

test('rustiq_open requires workdir when session has no cwd', async () => {
  const bin = await fakeBin()
  const m = mockCtx()
  m.agent.session.header.cwd = undefined
  apply(m.ctx, { binaryPath: bin })
  const open = m.tools.find((t) => t.name === 'rustiq_open')
  await assert.rejects(
    () => open.execute({}, { signal: AbortSignal.timeout(5_000), agent: m.agent }),
    /workdir required/,
  )
})

test('rustiq_open launches rustiq --inline in session cwd', async () => {
  const bin = await fakeBin()
  const m = mockCtx()
  apply(m.ctx, { binaryPath: bin })
  const open = m.tools.find((t) => t.name === 'rustiq_open')
  const out = await open.execute(
    {},
    { signal: AbortSignal.timeout(5_000), agent: m.agent },
  )
  assert.equal(out.sessionId, 'sess-1')
  assert.equal(out.workdir, '/repo')
  assert.equal(m.sends[0].req.text, `${bin} --inline`)
  assert.equal(m.sends[0].req.submit, true)
})

test('render clips at full utf-8 code point boundaries', () => {
  const m = mockCtx()
  apply(m.ctx, { maxResultBytes: 2 })
  const send = m.tools.find((t) => t.name === 'rustiq_send')
  const rendered = send.output.render({}, { viewport: 'a😀b' })
  assert.equal(rendered[0].text, 'a')
})
