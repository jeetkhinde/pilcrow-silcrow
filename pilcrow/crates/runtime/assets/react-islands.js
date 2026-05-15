(function () {
  "use strict";

  if (window.PilcrowReact && window.PilcrowReact.scan) {
    window.PilcrowReact.scan(document);
    return;
  }

  const mounted = new WeakSet();
  const scheduled = new WeakSet();
  const loadedCss = new Set();

  function camel(name) {
    return name.replace(/-([a-z])/g, (_, ch) => ch.toUpperCase());
  }

  function readProps(el) {
    const props = {};
    for (const attr of Array.from(el.attributes)) {
      if (attr.name.startsWith("data-prop-json-")) {
        const name = camel(attr.name.slice("data-prop-json-".length));
        try {
          props[name] = JSON.parse(attr.value);
        } catch (err) {
          console.warn("[pilcrow-react] invalid JSON prop", name, err);
          props[name] = null;
        }
        continue;
      }
      if (attr.name.startsWith("data-prop-")) {
        props[camel(attr.name.slice("data-prop-".length))] = attr.value;
      }
    }
    props.__pilcrowActionBase =
      el.getAttribute("data-pilcrow-action-base") ||
      props.__pilcrowActionBase ||
      window.location.pathname;
    return props;
  }

  function ensureCss(el) {
    const css = (el.getAttribute("data-css") || "").split(",").filter(Boolean);
    for (const href of css) {
      if (loadedCss.has(href)) continue;
      loadedCss.add(href);
      const link = document.createElement("link");
      link.rel = "stylesheet";
      link.href = href;
      document.head.appendChild(link);
    }
  }

  async function mount(el) {
    if (!el || mounted.has(el)) return;
    mounted.add(el);
    ensureCss(el);
    const src = el.getAttribute("data-src");
    if (!src) return;
    try {
      const mod = await import(src);
      const id = el.getAttribute("data-id");
      const registered = id && window.__pilcrowReactMounts && window.__pilcrowReactMounts[id];
      const mountFn = typeof mod.mount === "function" ? mod.mount : registered;
      if (typeof mountFn === "function") {
        mountFn(el, readProps(el));
      } else {
        console.warn("[pilcrow-react] module has no mount() export", src);
      }
    } catch (err) {
      mounted.delete(el);
      console.error("[pilcrow-react] failed to mount", src, err);
    }
  }

  function schedule(el) {
    if (!el || scheduled.has(el)) return;
    scheduled.add(el);
    const strategy = el.getAttribute("data-strategy") || "visible";
    if (strategy === "load" || strategy === "shell" || strategy === "ssr") {
      // shell/ssr: server-rendered HTML already present, hydrate immediately
      mount(el);
      return;
    }
    if (strategy === "idle") {
      if ("requestIdleCallback" in window) {
        requestIdleCallback(() => mount(el));
      } else {
        setTimeout(() => mount(el), 200);
      }
      return;
    }
    const observer = new IntersectionObserver(entries => {
      if (entries.some(entry => entry.isIntersecting)) {
        observer.disconnect();
        mount(el);
      }
    });
    observer.observe(el);
  }

  function scan(root) {
    const base = root && root.querySelectorAll ? root : document;
    if (base.matches && base.matches("[data-pilcrow-react]")) {
      schedule(base);
    }
    base.querySelectorAll("[data-pilcrow-react]").forEach(schedule);
  }

  window.PilcrowReact = {scan};
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", () => scan(document), {once: true});
  } else {
    scan(document);
  }
  document.addEventListener("silcrow:patched", event => {
    scan(event.detail && event.detail.target ? event.detail.target : document);
  });

  document.addEventListener("silcrow:load", event => {
    scan(event.detail && event.detail.target ? event.detail.target : document);
  });
})();
