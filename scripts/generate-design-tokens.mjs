import { readFile, writeFile } from 'node:fs/promises'

const source = new URL('../docs/design/tokens/base.json', import.meta.url)
const destination = new URL('../src/design-tokens.css', import.meta.url)
const tokens = JSON.parse(await readFile(source, 'utf8'))

const flatten = (value, path = []) => Object.entries(value).flatMap(([key, entry]) => {
  const nextPath = [...path, key]
  return typeof entry === 'object' ? flatten(entry, nextPath) : [[nextPath.join('-'), entry]]
})

const variables = flatten(tokens)
  .filter(([name]) => name !== '_comment')
  .map(([name, value]) => `  --${name}: ${value};`)
  .join('\n')

await writeFile(destination, `:root {\n${variables}\n}\n`)
