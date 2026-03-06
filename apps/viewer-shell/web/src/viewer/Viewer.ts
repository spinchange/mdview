export type HeadingSpan = {
  level: number;
  text: string;
  line_start: number;
  line_end: number;
  column_start: number;
  column_end: number;
};

export type RenderedDocument = {
  html: string;
  headings: HeadingSpan[];
  is_blank: boolean;
};

function slugify(value: string): string {
  return value
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9\s-]/g, "")
    .replace(/\s+/g, "-")
    .replace(/-+/g, "-");
}

function annotateHeadingNodes(root: ParentNode, headings: HeadingSpan[]): void {
  const headingElements = Array.from(
    root.querySelectorAll<HTMLHeadingElement>("h1, h2, h3, h4, h5, h6")
  );

  const slugCounts = new Map<string, number>();
  let cursor = 0;

  for (const element of headingElements) {
    const tagLevel = Number(element.tagName.slice(1));
    let matchIndex = -1;
    for (let i = cursor; i < headings.length; i += 1) {
      if (headings[i].level === tagLevel) {
        matchIndex = i;
        break;
      }
    }
    if (matchIndex < 0) {
      continue;
    }

    const heading = headings[matchIndex];
    cursor = matchIndex + 1;

    element.setAttribute("data-line-start", String(heading.line_start));
    element.setAttribute("data-line-end", String(heading.line_end));

    if (!element.id) {
      const base = slugify(heading.text) || `heading-${matchIndex + 1}`;
      const seen = slugCounts.get(base) ?? 0;
      slugCounts.set(base, seen + 1);
      element.id = seen > 0 ? `${base}-${seen + 1}` : base;
    }
  }
}

function buildToc(headings: HeadingSpan[]): HTMLElement {
  const nav = document.createElement("nav");
  nav.className = "mdv-toc";
  nav.setAttribute("aria-label", "Table of contents");

  const title = document.createElement("h2");
  title.className = "mdv-toc__title";
  title.textContent = "Contents";
  nav.appendChild(title);

  const list = document.createElement("ul");
  list.className = "mdv-toc__list";

  const slugCounts = new Map<string, number>();
  headings.forEach((heading, index) => {
    const item = document.createElement("li");
    item.className = "mdv-toc__item";
    item.style.paddingLeft = `${Math.max(0, heading.level - 1) * 12}px`;

    const link = document.createElement("a");
    link.className = "mdv-toc__link";
    const base = slugify(heading.text) || `heading-${index + 1}`;
    const seen = slugCounts.get(base) ?? 0;
    slugCounts.set(base, seen + 1);
    const id = seen > 0 ? `${base}-${seen + 1}` : base;
    link.href = `#${id}`;
    link.textContent = heading.text || `Heading ${index + 1}`;
    link.setAttribute("data-line-start", String(heading.line_start));

    item.appendChild(link);
    list.appendChild(item);
  });

  nav.appendChild(list);
  return nav;
}

export function renderDocument(container: HTMLElement, doc: RenderedDocument): void {
  container.innerHTML = "";

  const shell = document.createElement("section");
  shell.className = "mdv-shell";

  const toc = buildToc(doc.headings);
  const article = document.createElement("article");
  article.className = "mdv-content";

  if (doc.is_blank) {
    const empty = document.createElement("p");
    empty.className = "mdv-empty";
    empty.textContent = "This markdown file is empty.";
    article.appendChild(empty);
  } else {
    const template = document.createElement("template");
    template.innerHTML = doc.html;
    annotateHeadingNodes(template.content, doc.headings);
    article.appendChild(template.content.cloneNode(true));
  }

  shell.appendChild(toc);
  shell.appendChild(article);
  container.appendChild(shell);
}
