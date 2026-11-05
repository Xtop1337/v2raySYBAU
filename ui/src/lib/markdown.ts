// Minimal Markdown for release notes: everything is HTML-escaped first, then headings, lists,
// rules, code blocks, `code`, **bold**, *italic* and http(s) links are turned into tags.

const escape = (text: string) =>
  text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&#39;');

function inline(text: string): string {
  return escape(text)
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\*\*([^*]+)\*\*/g, '<b>$1</b>')
    .replace(/(^|[^*])\*([^*\s][^*]*)\*/g, '$1<i>$2</i>')
    .replace(/\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g, '<a href="$2">$1</a>');
}

export function renderMarkdown(source: string): string {
  const out: string[] = [];
  let list = false;
  let code: string[] | null = null;
  const closeList = () => {
    if (list) out.push('</ul>');
    list = false;
  };
  for (const raw of source.replace(/\r\n/g, '\n').split('\n')) {
    if (raw.trim().startsWith('```')) {
      if (code) {
        out.push(`<pre>${escape(code.join('\n'))}</pre>`);
        code = null;
      } else {
        closeList();
        code = [];
      }
      continue;
    }
    if (code) {
      code.push(raw);
      continue;
    }
    const line = raw.trim();
    const heading = line.match(/^(#{1,6})\s+(.*)$/);
    const item = line.match(/^[-*+]\s+(.*)$/);
    if (heading) {
      closeList();
      out.push(`<h4>${inline(heading[2])}</h4>`);
    } else if (item) {
      if (!list) out.push('<ul>');
      list = true;
      out.push(`<li>${inline(item[1])}</li>`);
    } else if (/^(-{3,}|\*{3,})$/.test(line)) {
      closeList();
      out.push('<hr>');
    } else if (line) {
      closeList();
      out.push(`<p>${inline(line)}</p>`);
    } else {
      closeList();
    }
  }
  closeList();
  if (code) out.push(`<pre>${escape(code.join('\n'))}</pre>`);
  return out.join('');
}
