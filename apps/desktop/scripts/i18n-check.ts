// Checks the translations (STEPS 8.8), so a new string can't be forgotten silently:
// - every t() / tn() / tr() / trn() / mark() string has a Hindi entry, with the same {names};
// - no English text sits in JSX (text, aria-label, title, placeholder, alt) outside t();
// - no Hindi entry is left over after its English string changed or went away.
// Run from apps/desktop: `node scripts/i18n-check.ts` (or with files, to check only those;
// then left-over entries aren't reported). tests/i18n.spec.ts runs it with the UI tests.
// `--loose` also lists other strings that look like English sentences (a review aid, not
// a gate: some are fine, like log text or values compared in code).
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import { dirname, join, relative } from 'node:path'
import { fileURLToPath } from 'node:url'
import ts from 'typescript'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const src = join(root, 'src')
const locales = join(src, 'locales')

/** Calls whose string arguments are English sentences to translate. */
const CALLS = new Set(['t', 'tn', 'tr', 'trn', 'mark'])
/** JSX attributes people read or hear. */
const ATTRS = new Set([
  'aria-label',
  'aria-description',
  'aria-roledescription',
  'aria-valuetext',
  'title',
  'placeholder',
  'alt',
  'label',
])
/** Not translated: brand names, standards and units (docs/07-design/I18N.md). */
const KEEP = [
  'Fuselane',
  'Fuse Send',
  'LocalSend',
  'AriaNg',
  'aria2',
  'Aria2',
  'yt-dlp',
  'ffmpeg',
  'BitTorrent',
  'GitHub',
  'Cloudflare',
  'Google',
  'YouTube',
  'Wi-Fi',
  'Ethernet',
  'SHA-256',
  'SHA256SUMS',
  'RSS',
  'Atom',
  'Metalink',
  'JSON-RPC',
  'DNS',
  'VPN',
  'USB',
  'HTTP',
  'HTTPS',
  'SOCKS5',
  'URL',
  'IP',
  'MP3',
  'HD',
  'QR',
  'MB/s',
  'Mbps',
  'KB',
  'MB',
  'GB',
  'TB',
  'MiB',
  'GiB',
  'Esc',
  'Space',
  'Ctrl',
  'Enter',
  'Tab',
]
const ENGLISH = /[A-Za-z]{2,}/

interface Problem {
  file: string
  line: number
  text: string
}

function walk(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name)
    if (statSync(p).isDirectory()) walk(p, out)
    else if (/\.tsx?$/.test(name)) out.push(p)
  }
  return out
}

/** Source files with UI text: not the catalogues, the layer itself or the demo backend. */
function sources(): string[] {
  return walk(src).filter((f) => {
    const r = relative(src, f)
    return !r.startsWith('locales') && r !== join('lib', 'i18n.ts') && !/lib[/\\]demo/.test(r)
  })
}

function parse(file: string): ts.SourceFile {
  const kind = file.endsWith('.tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS
  return ts.createSourceFile(file, readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true, kind)
}

function lineOf(sf: ts.SourceFile, node: ts.Node): number {
  return sf.getLineAndCharacterOfPosition(node.getStart(sf)).line + 1
}

function isText(n: ts.Node): n is ts.StringLiteral | ts.NoSubstitutionTemplateLiteral {
  return ts.isStringLiteral(n) || ts.isNoSubstitutionTemplateLiteral(n)
}

/** The catalogue: hi.ts (and any parts it is assembled from while work is in progress). */
function catalogue(
  problems: Problem[],
): Map<string, { value: string; file: string; line: number }> {
  const files = [join(locales, 'hi.ts')]
  const parts = join(locales, 'parts')
  if (existsSync(parts)) files.push(...walk(parts))
  const map = new Map<string, { value: string; file: string; line: number }>()
  for (const file of files) {
    const sf = parse(file)
    const visit = (n: ts.Node) => {
      if (ts.isPropertyAssignment(n) && (isText(n.name) || ts.isIdentifier(n.name))) {
        const key = n.name.text
        const line = lineOf(sf, n)
        if (!isText(n.initializer)) {
          problems.push({ file, line, text: `"${key}": the Hindi must be a plain string` })
        } else {
          const value = n.initializer.text
          const seen = map.get(key)
          if (seen && seen.value !== value)
            problems.push({
              file,
              line,
              text: `"${key}" is translated twice, differently (also ${relative(root, seen.file)}:${seen.line})`,
            })
          map.set(key, { value, file, line })
        }
      }
      ts.forEachChild(n, visit)
    }
    visit(sf)
  }
  return map
}

const names = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1] ?? '').sort()

/** Strings in `node` that aren't inside a translation call. */
function bareStrings(node: ts.Node, out: (ts.StringLiteral | ts.TemplateLiteral)[] = []) {
  if (ts.isCallExpression(node) && ts.isIdentifier(node.expression)) {
    if (CALLS.has(node.expression.text)) return out
  }
  if (ts.isBinaryExpression(node) && /^[=!]==?$/.test(node.operatorToken.getText())) return out
  if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) out.push(node)
  else if (ts.isTemplateExpression(node)) out.push(node)
  else ts.forEachChild(node, (c) => void bareStrings(c, out))
  return out
}

function staticText(n: ts.StringLiteral | ts.TemplateLiteral): string {
  if (ts.isTemplateExpression(n))
    return [n.head.text, ...n.templateSpans.map((s) => s.literal.text)].join(' ')
  return n.text
}

function looksEnglish(text: string): boolean {
  let rest = text
  for (const k of KEEP) rest = rest.split(k).join(' ')
  return ENGLISH.test(rest)
}

/** Text shown by JSX: a literal, `a ? 'x' : 'y'`, or `ok && 'x'`. */
function shownStrings(e: ts.Expression): (ts.StringLiteral | ts.TemplateLiteral)[] {
  if (ts.isParenthesizedExpression(e)) return shownStrings(e.expression)
  if (ts.isStringLiteral(e) || ts.isNoSubstitutionTemplateLiteral(e) || ts.isTemplateExpression(e))
    return [e]
  if (ts.isConditionalExpression(e))
    return [...shownStrings(e.whenTrue), ...shownStrings(e.whenFalse)]
  if (ts.isBinaryExpression(e) && e.operatorToken.kind === ts.SyntaxKind.AmpersandAmpersandToken)
    return shownStrings(e.right)
  if (ts.isBinaryExpression(e) && e.operatorToken.kind === ts.SyntaxKind.QuestionQuestionToken)
    return shownStrings(e.right)
  return []
}

function ignored(sf: ts.SourceFile, node: ts.Node): boolean {
  const line = lineOf(sf, node) - 1
  const lines = sf.text.split('\n')
  return [lines[line], lines[line - 1]].some((l) => l?.includes('i18n-ignore'))
}

/** Elements whose text is never translated: code, keys, and anything marked translate="no". */
function untranslatable(el: ts.JsxElement): boolean {
  const tag = el.openingElement.tagName.getText()
  if (tag === 'code' || tag === 'kbd' || tag === 'samp') return true
  return el.openingElement.attributes.properties.some(
    (a) =>
      ts.isJsxAttribute(a) &&
      a.name.getText() === 'translate' &&
      a.initializer !== undefined &&
      ts.isStringLiteral(a.initializer) &&
      a.initializer.text === 'no',
  )
}

/** Attributes that hold ids, classes or values, never text people read. */
const NONTEXT = new Set([
  'className',
  'key',
  'id',
  'type',
  'role',
  'name',
  'htmlFor',
  'href',
  'src',
  'inputMode',
  'autoComplete',
  'rel',
  'target',
  'pattern',
  'style',
  'translate',
  'lang',
  'dir',
])

/** Strings outside t() that look like English sentences ("Saved.", "Copy link"). */
function loose(sf: ts.SourceFile, report: (n: ts.Node, text: string) => void) {
  const visit = (n: ts.Node): void => {
    if (ts.isImportDeclaration(n) || ts.isExportDeclaration(n) || ts.isTypeNode(n)) return
    if (ts.isCallExpression(n) && ts.isIdentifier(n.expression) && CALLS.has(n.expression.text))
      return
    if (ts.isCallExpression(n) && /^console\./.test(n.expression.getText())) return
    if (ts.isJsxAttribute(n)) {
      const name = n.name.getText()
      if (NONTEXT.has(name) || ATTRS.has(name) || /^(data|aria)-/.test(name)) return
    }
    if (ts.isBinaryExpression(n) && /^[=!]==?$/.test(n.operatorToken.getText())) return
    if (ts.isCaseClause(n)) return ts.forEachChild(n.statements[0] ?? n, visit)
    if (ts.isElementAccessExpression(n)) return
    if (ts.isJsxText(n) || ts.isJsxExpression(n)) return ts.forEachChild(n, visit)
    if (
      ts.isStringLiteral(n) ||
      ts.isNoSubstitutionTemplateLiteral(n) ||
      ts.isTemplateExpression(n)
    ) {
      const p = n.parent
      if ((ts.isPropertyAssignment(p) && p.name === n) || ts.isLiteralTypeNode(p)) return
      if (ts.isJsxAttribute(p)) return
      const text = staticText(n).trim()
      if (
        /^[A-Z][a-z']/.test(text) &&
        /\s|[.…?!:]$/.test(text) &&
        looksEnglish(text) &&
        !ignored(sf, n)
      )
        report(n, `maybe English (loose): "${text}"`)
      return
    }
    ts.forEachChild(n, visit)
  }
  visit(sf)
}

function scan(
  file: string,
  used: Set<string>,
  cat: Map<string, { value: string }>,
  problems: Problem[],
  looseToo = false,
) {
  const sf = parse(file)
  const report = (node: ts.Node, text: string) =>
    problems.push({ file, line: lineOf(sf, node), text })

  const checkKey = (node: ts.Node, en: string, given: string[] | null) => {
    used.add(en)
    const entry = cat.get(en)
    if (!entry) report(node, `no Hindi for "${en}"`)
    else if (!entry.value.trim()) report(node, `empty Hindi for "${en}"`)
    else if (names(entry.value).join() !== names(en).join())
      report(node, `"${en}": the Hindi has {${names(entry.value)}}, the English {${names(en)}}`)
    else if (/[—–]/.test(entry.value)) report(node, `"${en}": no dashes in the Hindi (use , or :)`)
    if (given) {
      const missing = names(en).filter((n) => !given.includes(n))
      if (missing.length) report(node, `"${en}": nothing fills {${missing.join('}, {')}}`)
    }
  }

  /** The strings an argument can be: a literal, or either side of `a ? 'x' : 'y'`. */
  const literals = (call: ts.CallExpression, arg: ts.Expression | undefined): ts.Node[] => {
    if (!arg) return []
    if (ts.isParenthesizedExpression(arg)) return literals(call, arg.expression)
    if (isText(arg)) return [arg]
    if (ts.isConditionalExpression(arg))
      return [...literals(call, arg.whenTrue), ...literals(call, arg.whenFalse)]
    if (
      ts.isBinaryExpression(arg) &&
      (arg.operatorToken.kind === ts.SyntaxKind.QuestionQuestionToken ||
        arg.operatorToken.kind === ts.SyntaxKind.BarBarToken)
    )
      return [...literals(call, arg.left), ...literals(call, arg.right)]
    if (ts.isTemplateExpression(arg) || ts.isBinaryExpression(arg))
      report(arg, 'build the sentence with {names} in one string, not pieces')
    return [] // a variable: its strings are marked where they're defined
  }

  const visit = (node: ts.Node) => {
    if (ts.isCallExpression(node) && ts.isIdentifier(node.expression)) {
      const fn = node.expression.text
      if (CALLS.has(fn)) {
        const plural = fn === 'tn' || fn === 'trn'
        const forms = plural ? [node.arguments[1], node.arguments[2]] : [node.arguments[0]]
        const varsArg = node.arguments[plural ? 3 : 1]
        let given: string[] | null = plural ? ['n'] : []
        if (varsArg && ts.isObjectLiteralExpression(varsArg)) {
          for (const p of varsArg.properties) {
            if (ts.isSpreadAssignment(p)) given = null
            else if (given && p.name) given.push(p.name.getText().replace(/['"]/g, ''))
          }
        } else if (varsArg) given = null
        for (const f of forms)
          for (const lit of literals(node, f))
            if (isText(lit)) checkKey(lit, lit.text, fn === 'mark' ? null : given)
      }
    }
    if (ts.isJsxText(node)) {
      const text = node.text.trim()
      const parent = node.parent
      const skip = ts.isJsxElement(parent) && untranslatable(parent)
      if (text && !skip && looksEnglish(text) && !ignored(sf, node))
        report(node, `English in JSX: "${text.replace(/\s+/g, ' ')}"`)
    }
    if (ts.isJsxExpression(node) && node.expression && !ts.isJsxAttribute(node.parent)) {
      const parent = node.parent
      const skip = ts.isJsxElement(parent) && untranslatable(parent)
      if (!skip)
        for (const s of shownStrings(node.expression))
          if (looksEnglish(staticText(s)) && !ignored(sf, s))
            report(s, `English in JSX: "${staticText(s)}"`)
    }
    if (ts.isJsxAttribute(node) && ATTRS.has(node.name.getText()) && node.initializer) {
      const init = node.initializer
      const strings = ts.isStringLiteral(init)
        ? [init]
        : ts.isJsxExpression(init) && init.expression
          ? bareStrings(init.expression)
          : []
      for (const s of strings)
        if (looksEnglish(staticText(s)) && !ignored(sf, s))
          report(s, `English in ${node.name.getText()}: "${staticText(s)}"`)
    }
    ts.forEachChild(node, visit)
  }
  visit(sf)
  if (looseToo) loose(sf, report)
}

function main() {
  const looseToo = process.argv.includes('--loose')
  const args = process.argv.slice(2).filter((a) => a !== '--loose')
  const problems: Problem[] = []
  const cat = catalogue(problems)
  const used = new Set<string>()
  const files = args.length ? args.map((a) => join(process.cwd(), a)) : sources()
  for (const f of files) scan(f, used, cat, problems, looseToo)
  if (!args.length)
    for (const [key, e] of cat)
      if (!used.has(key))
        problems.push({ file: e.file, line: e.line, text: `left over (not in the code): "${key}"` })
  for (const p of problems) console.log(`${relative(root, p.file)}:${p.line}: ${p.text}`)
  console.log(
    problems.length
      ? `\n${problems.length} translation problem(s). See docs/07-design/I18N.md.`
      : `Translations OK: ${used.size} strings, all in Hindi.`,
  )
  process.exitCode = problems.length ? 1 : 0
}

main()
