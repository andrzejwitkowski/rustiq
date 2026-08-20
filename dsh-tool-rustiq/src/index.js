import { launchCommand, resolveRustiqBinary } from './binary.js'

export const name = 'tool-rustiq'
export const inject = ['terminals', 'tools', 'systemPrompt']

export const DEFAULT_MAX_RESULT_BYTES = 256 * 1024
export const GUIDANCE = `For git diff review and inline comments, use rustiq_* (not bash rustiq). rustiq_open starts the TUI; rustiq_send keys (j/k navigate, Enter select baseline, c comment, V export, q quit); rustiq_read the screen; rustiq_close when done. Track sessionId. After V, comment text is at .rustiq/export.txt in the repo.`

function requireAgent(agent) {
  if (agent === undefined) throw new Error('rustiq tools require an initiating agent')
  return agent
}

function sessionId(args) {
  if (typeof args.sessionId !== 'string' || args.sessionId.length === 0) {
    throw new Error('sessionId must be a non-empty string')
  }
  return args.sessionId
}

function clip(text, maxBytes) {
  const buf = Buffer.from(text, 'utf8')
  if (buf.length <= maxBytes) return text
  return buf.subarray(0, maxBytes).toString('utf8')
}

function textBlocks(text, maxBytes) {
  return [{ type: 'text', text: clip(text, maxBytes) }]
}

function terminalCard(result) {
  if (result.isError) return undefined
  const block = result.content?.[0]
  return block?.type === 'text' ? { card: 'terminal', output: block.text } : undefined
}

async function ptySend(ctx, owner, id, req, signal) {
  const sent = await ctx.terminals.startSend(owner, id, { ...req, signal }).done
  if (signal.aborted) throw new Error('rustiq send aborted')
  return { viewport: sent.viewport, waitReason: sent.waitReason }
}

export function apply(ctx, config = {}) {
  const maxResultBytes = config.maxResultBytes ?? DEFAULT_MAX_RESULT_BYTES
  const defaultBinary = config.binaryPath ?? 'rustiq'

  ctx.systemPrompt.section({ name: 'tool:rustiq', order: 107, text: GUIDANCE })

  ctx.tools.register({
    name: 'rustiq_open',
    description: 'Start a rustiq TUI review session in a persistent PTY owned by this agent. Use for git diff browsing and inline comments.',
    parameters: {
      workdir: { type: 'string', description: 'Git repo to review. Defaults to the session workspace cwd.' },
      binaryPath: { type: 'string', description: 'rustiq executable name or absolute path. Defaults to plugin config / PATH.' },
      name: { type: 'string', description: 'Optional terminal display name (default rustiq).' },
    },
    output: {
      schema: {
        type: 'object',
        additionalProperties: false,
        properties: {
          sessionId: { type: 'string', required: true },
          binary: { type: 'string', required: true },
          workdir: { type: 'string', required: true },
          viewport: { type: 'string', required: true },
          waitReason: { type: 'string', required: true },
        },
      },
      render: (_args, value) => textBlocks(`opened rustiq session ${value.sessionId} in ${value.workdir}\n${value.viewport}`, maxResultBytes),
    },
    async execute(args, exec) {
      const owner = requireAgent(exec.agent)
      const workdir = args.workdir || owner.session?.header?.cwd
      if (typeof workdir !== 'string' || workdir.length === 0) {
        throw new Error('workdir required')
      }
      const binary = await resolveRustiqBinary(args.binaryPath || defaultBinary)
      const spawned = await ctx.terminals.spawn(owner, {
        type: 'shell',
        name: args.name || 'rustiq',
        cwd: workdir,
      }, exec.signal)
      const id = spawned.sessionId
      try {
        const sent = await ptySend(ctx, owner, id, { text: launchCommand(binary), submit: true }, exec.signal)
        return { sessionId: id, binary, workdir, ...sent }
      } catch (err) {
        await ctx.terminals.kill(owner, id).catch(() => {})
        throw err
      }
    },
    presentCall: (args) => ({
      card: 'terminal',
      title: 'rustiq',
      description: `Open rustiq in ${args.workdir || 'session cwd'}`,
      cwd: args.workdir,
    }),
    presentResult: (_args, result) => {
      const card = terminalCard(result)
      return card ? { ...card, title: 'rustiq' } : undefined
    },
  })

  ctx.tools.register({
    name: 'rustiq_send',
    description: 'Send keystrokes to an open rustiq TUI. Set submit false for raw keys (j/k/c/V/q). Set submit true to press Enter after text (comment save, baseline select).',
    parameters: {
      sessionId: { type: 'string', required: true, description: 'Id from rustiq_open.' },
      text: { type: 'string', required: true, description: 'UTF-8 keys to write (e.g. j, k, V, q, or comment text).' },
      submit: { type: 'boolean', description: 'Append Enter after text (default false for TUI keys).' },
    },
    output: {
      schema: {
        type: 'object',
        additionalProperties: false,
        properties: {
          sessionId: { type: 'string', required: true },
          viewport: { type: 'string', required: true },
          waitReason: { type: 'string', required: true },
        },
      },
      render: (_args, value) => textBlocks(value.viewport, maxResultBytes),
    },
    async execute(args, exec) {
      const owner = requireAgent(exec.agent)
      const id = sessionId(args)
      const sent = await ptySend(ctx, owner, id, { text: args.text, submit: args.submit === true }, exec.signal)
      return { sessionId: id, ...sent }
    },
    presentCall: (args) => ({
      card: 'terminal',
      title: args.text || '(keys)',
      description: `rustiq ${args.sessionId}`,
    }),
    presentResult: (_args, result) => terminalCard(result),
  })

  ctx.tools.register({
    name: 'rustiq_read',
    description: 'Read the current rustiq TUI screen without sending input.',
    parameters: {
      sessionId: { type: 'string', required: true, description: 'Id from rustiq_open.' },
      offset: { type: 'number', description: 'Newest-relative line offset (default 0).' },
      count: { type: 'number', description: 'Line count (default backend cap).' },
    },
    output: {
      schema: {
        type: 'object',
        additionalProperties: false,
        properties: {
          sessionId: { type: 'string', required: true },
          text: { type: 'string', required: true },
          totalLines: { type: 'integer', required: true },
          truncated: { type: 'boolean', required: true },
        },
      },
      render: (_args, value) => textBlocks(value.text, maxResultBytes),
    },
    async execute(args, exec) {
      const owner = requireAgent(exec.agent)
      const id = sessionId(args)
      const opts = {}
      if (args.offset !== undefined) opts.offset = args.offset
      if (args.count !== undefined) opts.count = args.count
      const page = ctx.terminals.read(owner, id, opts)
      return {
        sessionId: id,
        text: page.text,
        totalLines: page.totalLines,
        truncated: page.truncated === true,
      }
    },
    presentCall: (args) => ({ card: 'generic', title: `Read rustiq ${args.sessionId}`, kind: 'read' }),
  })

  ctx.tools.register({
    name: 'rustiq_close',
    description: 'Close a rustiq TUI session and wait until its process tree exits.',
    parameters: {
      sessionId: { type: 'string', required: true, description: 'Id from rustiq_open.' },
    },
    output: {
      schema: {
        type: 'object',
        additionalProperties: false,
        properties: {
          sessionId: { type: 'string', required: true },
          outcome: { type: 'string', required: true, enum: ['closed', 'already-closing'] },
        },
      },
      render: (_args, value) => textBlocks(
        value.outcome === 'closed'
          ? `closed rustiq session ${value.sessionId}`
          : `rustiq session ${value.sessionId} was already closing`,
        maxResultBytes,
      ),
    },
    async execute(args, exec) {
      const owner = requireAgent(exec.agent)
      const id = sessionId(args)
      const closed = await ctx.terminals.kill(owner, id)
      return { sessionId: id, outcome: closed ? 'closed' : 'already-closing' }
    },
    presentCall: (args) => ({ card: 'generic', title: `Close rustiq ${args.sessionId}`, kind: 'delete' }),
  })
}
