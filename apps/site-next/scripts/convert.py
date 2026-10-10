"""One-off port of the plain-HTML site (apps/site) to Next.js pages.

For each page it writes app/<route>/page.tsx with:
  - `metadata` built from the page's <head> (title, description, canonical,
    robots, Open Graph, Twitter),
  - the page's JSON-LD, plus what the old Vite seo() plugin generated
    (WebSite on home, BreadcrumbList on listed pages, FAQPage on /faq/),
  - the <main> markup as JSX: icons rendered inline, relative links made
    absolute, internal links as next/link.
It also writes components/nav-markup.tsx and components/footer-markup.tsx
from the partials. Run from apps/site-next:  python3 scripts/convert.py
"""
import html as htmllib
import json
import os
import re
from urllib.parse import urljoin

SITE = 'https://fuselane.app'
SRC = '../site'
LISTED = [
    ('/', 'Fuselane'), ('/download/', 'Download'), ('/guide/', 'Guide'), ('/faq/', 'Questions'),
    ('/idm-alternative/', 'IDM alternative'), ('/combine-internet/', 'Combine internet connections'),
    ('/send-large-files/', 'Send large files'), ('/support/', 'Support'), ('/privacy/', 'Privacy'), ('/terms/', 'Terms'),
]
PAGES = {
    '/': 'index.html', '/download/': 'download/index.html', '/guide/': 'guide/index.html', '/faq/': 'faq/index.html',
    '/support/': 'support/index.html', '/privacy/': 'privacy/index.html', '/terms/': 'terms/index.html',
    '/idm-alternative/': 'idm-alternative/index.html', '/combine-internet/': 'combine-internet/index.html',
    '/send-large-files/': 'send-large-files/index.html', '/s/': 's/index.html',
}
# Which page script runs where (lib/legacy/*).
SCRIPTS = {'/': 'home', '/download/': 'download', '/support/': 'support', '/s/': 'receive'}

RENAME = {
    'class': 'className', 'for': 'htmlFor', 'tabindex': 'tabIndex', 'srcset': 'srcSet',
    'stroke-width': 'strokeWidth', 'stroke-linecap': 'strokeLinecap', 'stroke-linejoin': 'strokeLinejoin',
    'stroke-dasharray': 'strokeDasharray', 'stroke-dashoffset': 'strokeDashoffset', 'text-anchor': 'textAnchor',
    'fill-rule': 'fillRule', 'clip-rule': 'clipRule', 'stop-color': 'stopColor', 'xlink:href': 'xlinkHref',
    'preserveaspectratio': 'preserveAspectRatio', 'viewbox': 'viewBox', 'pathlength': 'pathLength',
    'checked': 'defaultChecked', 'autocomplete': 'autoComplete', 'readonly': 'readOnly', 'maxlength': 'maxLength',
    'colspan': 'colSpan', 'rowspan': 'rowSpan', 'crossorigin': 'crossOrigin', 'playsinline': 'playsInline',
    'autoplay': 'autoPlay', 'fetchpriority': 'fetchPriority',
}
VOID = {'img', 'input', 'source', 'br', 'hr', 'meta', 'link'}


def style_obj(css):
    parts = []
    for rule in css.split(';'):
        if ':' not in rule:
            continue
        k, v = (x.strip() for x in rule.split(':', 1))
        key = k if k.startswith('--') else re.sub(r'-(\w)', lambda m: m.group(1).upper(), k)
        parts.append(f"{json.dumps(key)}: {json.dumps(v)}")
    return '{{ ' + ', '.join(parts) + ' }}'


def resolve(url, page):
    """Relative URL in the plain site -> absolute path for Next."""
    if re.match(r'^(https?:|mailto:|tel:|fuselane:|#|data:)', url) or url.startswith('/'):
        return url
    out = urljoin(page, url)
    return out


def convert_attrs(tag, raw, page, ctx):
    out = []
    link = False
    for m in re.finditer(r'([\w:-]+)(?:\s*=\s*"([^"]*)")?', raw):
        name, val = m.group(1), m.group(2)
        lname = name.lower()
        if val is None:  # boolean attribute
            if lname.startswith('data-') or lname.startswith('aria-'):
                out.append(f'{name}=""')
            else:
                out.append(RENAME.get(lname, name))
            continue
        if lname == 'style':
            out.append('style=' + style_obj(val))
            continue
        if lname == 'data-page':
            ctx['current'] = True
            out.append(f'aria-current={{current === {json.dumps(val)} ? "page" : undefined}}')
            continue
        if lname in ('href', 'src'):
            val = val.replace('{{base}}', '/')
            val = resolve(val, page)
            # Links whose href the page script rewrites (data-dl, data-open)
            # stay plain <a>: next/link would navigate to its own href.
            if lname == 'href' and tag == 'a' and val.startswith('/') and not re.search(r'\bdata-(dl|open)\b', raw):
                link = True
        if lname == 'srcset':
            val = ', '.join(
                ' '.join([resolve(p.strip().split(' ')[0].replace('{{base}}', '/'), page)] + p.strip().split(' ')[1:])
                for p in val.split(','))
        val = htmllib.unescape(val)
        if lname in ('tabindex', 'colspan', 'rowspan', 'maxlength') and re.fullmatch(r'-?\d+', val):
            out.append(f'{RENAME.get(lname, name)}={{{val}}}')
            continue
        # A JSX string attribute doesn't decode escapes, so write real
        # characters; values that need escaping go in as expressions.
        if '"' in val or '\\' in val:
            out.append(f'{RENAME.get(lname, name)}={{{json.dumps(val, ensure_ascii=False)}}}')
        else:
            out.append(f'{RENAME.get(lname, name)}="{val}"')
    return out, link


def to_jsx(markup, page, ctx):
    markup = re.sub(r'<!--.*?-->', '', markup, flags=re.S)
    # Prettier wraps tags as `<a ...\n  >` and `</i\n>`: close them up first.
    markup = re.sub(r'(<[^<>]*?)\s+>', r'\1>', markup)
    # Icons render into the HTML.
    # (The plain site swapped the whole <i> for the icon, dropping any class.)
    markup = re.sub(r'<i data-icon="([\w-]+)"[^>]*></i>', lambda m: f'<Icon name="{m.group(1)}" />', markup)
    ctx['icon'] = '<Icon ' in markup
    stack = []

    def tag(m):
        closing, name, raw, selfclose = m.group(1), m.group(2), m.group(3) or '', m.group(4)
        if name == 'Icon':
            return m.group(0)
        if closing:
            opened = stack.pop() if stack else name
            return '</Link>' if opened == 'Link' else f'</{name}>'
        attrs, link = convert_attrs(name, raw, page, ctx)
        out_name = 'Link' if link else name
        if link:
            ctx['link'] = True
        a = (' ' + ' '.join(attrs)) if attrs else ''
        if name.lower() in VOID or selfclose:
            return f'<{out_name}{a} />'
        stack.append(out_name)
        return f'<{out_name}{a}>'

    markup = re.sub(r'<(/)?([a-zA-Z][\w-]*)((?:\s+[\w:-]+(?:\s*=\s*"[^"]*")?)*)\s*(/)?>', tag, markup)
    return keep_spaces(markup)


def keep_spaces(markup):
    """HTML renders a line break between inline pieces as a space; JSX drops
    it. Turn every whitespace run that holds a newline (outside tags) into an
    explicit {' '} so text reads exactly as before."""
    parts = re.split(r'(<[^>]*>)', markup)
    for i in range(0, len(parts), 2):
        parts[i] = re.sub(r'[ \t]*\n\s*', lambda m: "{' '}\n", parts[i])
    return ''.join(parts)


def head_meta(src):
    head = re.search(r'<head>(.*?)</head>', src, re.S).group(1)

    def meta(attr, key):
        m = re.search(rf'<meta\s+{attr}="{re.escape(key)}"\s+content="([^"]*)"', head, re.S)
        return htmllib.unescape(m.group(1)) if m else None

    title = htmllib.unescape(re.sub(r'\s+', ' ', re.search(r'<title>(.*?)</title>', head, re.S).group(1)).strip())
    canonical = re.search(r'<link rel="canonical" href="([^"]+)"', head)
    md = {'title': {'absolute': title}}
    if (d := meta('name', 'description')):
        md['description'] = re.sub(r'\s+', ' ', d)
    if canonical:
        md['alternates'] = {'canonical': canonical.group(1)}
    if (r := meta('name', 'robots')):
        md['robots'] = r
    og = {}
    for k, key in [('type', 'og:type'), ('siteName', 'og:site_name'), ('locale', 'og:locale'), ('url', 'og:url'),
                   ('title', 'og:title'), ('description', 'og:description')]:
        if (v := meta('property', key)):
            og[k] = re.sub(r'\s+', ' ', v)
    if (img := meta('property', 'og:image')):
        image = {'url': img}
        for k, key in [('width', 'og:image:width'), ('height', 'og:image:height'), ('alt', 'og:image:alt')]:
            if (v := meta('property', key)):
                image[k] = int(v) if k != 'alt' else re.sub(r'\s+', ' ', v)
        og['images'] = [image]
    if og:
        md['openGraph'] = og
    tw = {}
    for k, key in [('card', 'twitter:card'), ('title', 'twitter:title'), ('description', 'twitter:description')]:
        if (v := meta('name', key)):
            tw[k] = re.sub(r'\s+', ' ', v)
    if (v := meta('name', 'twitter:image')):
        tw['images'] = [v]
    if tw:
        md['twitter'] = tw
    lds = [json.loads(b) for b in re.findall(r'<script type="application/ld\+json">(.*?)</script>', head, re.S)]
    return md, lds


def text(h):
    """Plain text for JSON-LD: block tags become spaces, inline tags vanish,
    and no space is left before punctuation that ends a phrase."""
    h = re.sub(r'</?(p|li|ul|ol|div|br|h\d|dd|dt|table|tr|td|th)\b[^>]*>', ' ', h)
    h = re.sub(r'<[^>]+>', '', h)
    h = re.sub(r'\s+', ' ', htmllib.unescape(h).replace('\u00a0', ' ')).strip()
    return re.sub(r'\s+([,.;:!?)])(?=\s|$)', r'\1', h)


def generated_ld(path, main):
    out = []
    if path == '/':
        out.append({'@context': 'https://schema.org', '@type': 'WebSite', 'name': 'Fuselane', 'url': f'{SITE}/',
                    'publisher': {'@type': 'Organization', 'name': 'Fuselane', 'url': f'{SITE}/', 'logo': f'{SITE}/icon-512.png',
                                  'sameAs': ['https://github.com/ArshPunisher/fuselane']}})
    page = next((n for p, n in LISTED if p == path), None)
    if page and path != '/':
        out.append({'@context': 'https://schema.org', '@type': 'BreadcrumbList', 'itemListElement': [
            {'@type': 'ListItem', 'position': 1, 'name': 'Fuselane', 'item': f'{SITE}/'},
            {'@type': 'ListItem', 'position': 2, 'name': page, 'item': f'{SITE}{path}'}]})
    if path == '/faq/':
        qs = [{'@type': 'Question', 'name': text(q), 'acceptedAnswer': {'@type': 'Answer', 'text': text(a)}}
              for q, a in re.findall(r'<details[^>]*>\s*<summary>([\s\S]*?)</summary>([\s\S]*?)</details>', main)]
        out.append({'@context': 'https://schema.org', '@type': 'FAQPage', 'mainEntity': qs})
    return out


def indent(s, n):
    return '\n'.join((' ' * n + l) if l.strip() else '' for l in s.strip('\n').split('\n'))


def write_page(path, file):
    src = open(os.path.join(SRC, file)).read()
    md, lds = head_meta(src)
    main_raw = re.search(r'<main([^>]*)>(.*?)</main>', src, re.S)
    main_attrs, inner = main_raw.group(1), main_raw.group(2)
    lds += generated_ld(path, inner)
    ctx = {}
    jsx = to_jsx(inner, path, ctx)
    m_attrs, _ = convert_attrs('main', main_attrs, path, ctx)
    imports = ["import type { Metadata } from 'next';"]
    if ctx.get('link'):
        imports.append("import Link from 'next/link';")
    if ctx.get('icon'):
        imports.append("import { Icon } from '@/components/icon';")
    imports.append("import { JsonLd } from '@/components/json-ld';")
    script = SCRIPTS.get(path)
    imports.append("import { PageScript } from '@/components/page-script';")
    route_dir = 'app' + path.rstrip('/')
    os.makedirs(route_dir, exist_ok=True)
    main_open = '<main' + ((' ' + ' '.join(m_attrs)) if m_attrs else '') + '>'
    body = f"""// Ported from apps/site/{file} by scripts/convert.py.
{chr(10).join(imports)}

export const metadata: Metadata = {json.dumps(md, indent=2, ensure_ascii=False)};

const LD: object[] = {json.dumps(lds, ensure_ascii=False)};

export default function Page() {{
  return (
    <>
      <JsonLd data={{LD}} />
      {main_open}
{indent(jsx, 8)}
      </main>
{('      <PageScript name="' + script + '" />') if script else '      <PageScript />'}
    </>
  );
}}
"""
    open(os.path.join(route_dir, 'page.tsx'), 'w').write(body)
    return md['title']['absolute']


def write_partial(name, comp):
    src = open(os.path.join(SRC, 'partials', f'{name}.html')).read()
    ctx = {}
    jsx = to_jsx(src, '/', ctx)
    imports = []
    if ctx.get('link'):
        imports.append("import Link from 'next/link';")
    if ctx.get('icon'):
        imports.append("import { Icon } from './icon';")
    sig = '{ current }: { current: string }' if ctx.get('current') else ''
    open(f'components/{name}-markup.tsx', 'w').write(f"""// Ported from apps/site/partials/{name}.html by scripts/convert.py.
{chr(10).join(imports)}

export function {comp}({sig}) {{
  return (
    <>
{indent(jsx, 6)}
    </>
  );
}}
""")


if __name__ == '__main__':
    for path, file in PAGES.items():
        print(path, '->', write_page(path, file))
    write_partial('nav', 'NavMarkup')
    write_partial('footer', 'FooterMarkup')
