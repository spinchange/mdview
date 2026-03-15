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

export type RenderDocumentOptions = {
  quickEditEnabled?: boolean;
  onJumpToLine?: (lineNumber: number) => void;
};

type InvokeFn = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

type ViewerDom = {
  shell: HTMLElement;
  toc: HTMLElement;
  article: HTMLElement;
};

type ScrollSnapshot = {
  scrollY: number;
  anchorIndex: number;
  anchorTop: number;
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

function buildToc(
  headings: HeadingSpan[],
  options: RenderDocumentOptions
): HTMLElement {
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
    if (options.quickEditEnabled) {
      link.title = `Jump to line ${heading.line_start} in editor`;
      link.addEventListener("click", (event) => {
        event.preventDefault();
        options.onJumpToLine?.(heading.line_start);
      });
    }

    item.appendChild(link);
    list.appendChild(item);
  });

  nav.appendChild(list);
  return nav;
}

function syncToc(
  toc: HTMLElement,
  headings: HeadingSpan[],
  options: RenderDocumentOptions
): void {
  const nextToc = buildToc(headings, options);
  toc.replaceChildren(...Array.from(nextToc.childNodes));
  Array.from(nextToc.attributes).forEach((attribute) => {
    toc.setAttribute(attribute.name, attribute.value);
  });
}

function captureScrollSnapshot(article: HTMLElement): ScrollSnapshot | null {
  const children = Array.from(article.children).filter(
    (child): child is HTMLElement => child instanceof HTMLElement
  );
  if (children.length === 0) {
    return null;
  }

  const anchorIndex = children.findIndex((child) => child.getBoundingClientRect().bottom >= 0);
  const safeIndex = anchorIndex >= 0 ? anchorIndex : children.length - 1;
  const anchor = children[safeIndex];

  return {
    scrollY: window.scrollY,
    anchorIndex: safeIndex,
    anchorTop: anchor.getBoundingClientRect().top,
  };
}

function restoreScrollSnapshot(article: HTMLElement, snapshot: ScrollSnapshot | null): void {
  if (!snapshot) {
    return;
  }

  const children = Array.from(article.children).filter(
    (child): child is HTMLElement => child instanceof HTMLElement
  );
  if (children.length === 0) {
    window.scrollTo({ top: snapshot.scrollY, behavior: "instant" });
    return;
  }

  const anchor = children[Math.min(snapshot.anchorIndex, children.length - 1)];
  const delta = anchor.getBoundingClientRect().top - snapshot.anchorTop;
  if (Math.abs(delta) < 1) {
    return;
  }

  window.scrollTo({
    top: snapshot.scrollY + delta,
    behavior: "instant",
  });
}

async function openExternalLink(url: string): Promise<void> {
  const tauriWindow = window as Window & {
    __TAURI__?: {
      core?: { invoke?: InvokeFn };
      tauri?: { invoke?: InvokeFn };
    };
  };

  const invoke =
    tauriWindow.__TAURI__?.core?.invoke ??
    tauriWindow.__TAURI__?.tauri?.invoke;

  if (!invoke) {
    window.open(url, "_blank", "noopener,noreferrer");
    return;
  }

  await invoke("open_external_link", { url });
}

function ensureViewerDom(container: HTMLElement): ViewerDom {
  const existingShell = container.querySelector<HTMLElement>(":scope > .mdv-shell");
  const existingToc = existingShell?.querySelector<HTMLElement>(":scope > .mdv-toc");
  const existingArticle = existingShell?.querySelector<HTMLElement>(":scope > .mdv-content");

  if (existingShell && existingToc && existingArticle) {
    return {
      shell: existingShell,
      toc: existingToc,
      article: existingArticle,
    };
  }

  container.replaceChildren();

  const shell = document.createElement("section");
  shell.className = "mdv-shell";

  const toc = document.createElement("nav");
  toc.className = "mdv-toc";

  const article = document.createElement("article");
  article.className = "mdv-content";

  shell.appendChild(toc);
  shell.appendChild(article);
  container.appendChild(shell);

  return { shell, toc, article };
}
export function renderDocument(
  container: HTMLElement,
  doc: RenderedDocument,
  options: RenderDocumentOptions = {}
): void {
  const { toc, article } = ensureViewerDom(container);
  const scrollSnapshot = captureScrollSnapshot(article);

  syncToc(toc, doc.headings, options);
  if (doc.is_blank) {
    const empty = document.createElement("p");
    empty.className = "mdv-empty";
    empty.textContent = "This markdown file is empty.";
    article.replaceChildren(empty);
  } else {
    const template = document.createElement("template");
    template.innerHTML = doc.html;
    annotateHeadingNodes(template.content, doc.headings);
    article.replaceChildren(template.content.cloneNode(true));
  }

  article.classList.toggle("mdv-content--jumpable", !!options.quickEditEnabled);
  article.onclick = null;

  article.onclick = (event) => {
    const target = event.target;
    if (!(target instanceof Element)) {
      return;
    }

    const link = target.closest<HTMLAnchorElement>("a[href]");
    if (link) {
      const href = link.getAttribute("href") ?? "";

      if (href.startsWith("#")) {
        const targetElement = article.querySelector<HTMLElement>(href);
        if (targetElement) {
          event.preventDefault();
          targetElement.scrollIntoView({ behavior: "smooth" });
        }
        return;
      }

      const protocol = link.protocol;

      if (protocol === "http:" || protocol === "https:" || protocol === "mailto:") {
        event.preventDefault();
        void openExternalLink(link.href).catch(console.error);
        return;
      }
    }

    if (!options.quickEditEnabled) {
      return;
    }

    const heading = target.closest<HTMLHeadingElement>("h1, h2, h3, h4, h5, h6");
    if (!heading) {
      return;
    }

    const lineStart = Number(heading.getAttribute("data-line-start"));
    if (Number.isFinite(lineStart) && lineStart > 0) {
      event.preventDefault();
      options.onJumpToLine?.(lineStart);
    }
  };

  restoreScrollSnapshot(article, scrollSnapshot);
}
