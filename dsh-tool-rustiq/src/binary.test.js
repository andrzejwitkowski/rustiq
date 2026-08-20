import assert from 'node:assert/strict'
import { test } from 'node:test'
import { launchCommand, resolveRustiqBinary, shellQuote } from './binary.js'

test('shellQuote leaves simple paths alone', () => {
  assert.equal(shellQuote('/usr/local/bin/rustiq'), '/usr/local/bin/rustiq')
})

test('shellQuote wraps spaces', () => {
  assert.equal(shellQuote('/opt/my bin/rustiq'), `'/opt/my bin/rustiq'`)
})

test('launchCommand adds --inline', () => {
  assert.equal(launchCommand('/usr/bin/rustiq'), '/usr/bin/rustiq --inline')
})

test('resolveRustiqBinary rejects empty', async () => {
  await assert.rejects(() => resolveRustiqBinary('  '), /non-empty/)
})

test('resolveRustiqBinary rejects missing absolute path', async () => {
  await assert.rejects(
    () => resolveRustiqBinary('/no/such/rustiq-binary'),
    /not found or not executable/,
  )
})
