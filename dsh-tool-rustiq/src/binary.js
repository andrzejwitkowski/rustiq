import { access, constants } from 'node:fs/promises'
import { execFile } from 'node:child_process'
import { isAbsolute } from 'node:path'
import { promisify } from 'node:util'

const execFileAsync = promisify(execFile)

export function shellQuote(path) {
  if (/^[A-Za-z0-9_./+-]+$/.test(path)) return path
  return `'${path.replace(/'/g, `'\\''`)}'`
}

export async function resolveRustiqBinary(binaryPath) {
  const name = binaryPath.trim()
  if (!name) throw new Error('rustiq binaryPath must be a non-empty string')
  if (isAbsolute(name)) {
    try {
      await access(name, constants.X_OK)
    } catch {
      throw new Error(`rustiq binary not found or not executable: ${name}`)
    }
    return name
  }
  try {
    const { stdout } = await execFileAsync('which', [name])
    const resolved = stdout.trim()
    if (resolved) return resolved
  } catch { /* which failed */ }
  throw new Error(`rustiq binary not found on PATH: ${name}`)
}

export function launchCommand(resolvedBinary) {
  return `${shellQuote(resolvedBinary)} --inline`
}
