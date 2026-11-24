import { useEffect, useRef, useState } from 'react';

interface SandboxedEmailFrameProps {
  html: string;
  allowRemoteAssets: boolean;
}

/**
 * Renders sanitized email HTML inside a strictly sandboxed iframe:
 * - `sandbox="allow-same-origin"` with ALL scripts disabled (`allow-scripts` is NEVER set)
 * - Injected strict CSP `<meta>` inside `srcDoc` locking down `script-src 'none'`,
 *   `object-src 'none'`, `form-action 'none'`, and `img-src data:` unless the user explicitly
 *   toggled "Load Remote Assets" for this message.
 * - Zero-script height measurement via parent `contentDocument` ResizeObserver.
 */
export function SandboxedEmailFrame({ html, allowRemoteAssets }: SandboxedEmailFrameProps) {
  const iframeRef = useRef<HTMLIFrameElement | null>(null);
  const [height, setHeight] = useState(140);

  const imgCsp = allowRemoteAssets ? "img-src data: https: http:;" : "img-src data:;";
  const srcDoc = `<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'none'; object-src 'none'; form-action 'none'; frame-src 'none'; style-src 'unsafe-inline'; ${imgCsp} font-src data:;" />
  <base target="_blank" />
  <style>
    :root { color-scheme: dark; }
    * { box-sizing: border-box; }
    html, body {
      margin: 0;
      padding: 0;
      background: transparent;
      color: #d8dee9;
      font-family: "Inter", -apple-system, BlinkMacSystemFont, "SF Pro Text", sans-serif;
      font-size: 13.5px;
      line-height: 1.65;
      overflow-x: auto;
      overflow-y: hidden;
      word-break: break-word;
    }
    p { margin: 0 0 11px 0; }
    p:last-child { margin-bottom: 0; }
    a { color: #34d399; text-decoration: none; border-bottom: 1px solid rgba(52, 211, 153, 0.3); }
    a:hover { color: #6ee7b7; border-bottom-color: #6ee7b7; }
    strong { color: #f3f6fb; font-weight: 600; }
    code {
      font-family: "JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, monospace;
      font-size: 12px;
      background: rgba(255,255,255,0.06);
      border: 1px solid rgba(255,255,255,0.08);
      padding: 1px 5px;
      border-radius: 4px;
      color: #e2e8f0;
    }
    ul, ol { margin: 8px 0 12px 0; padding-left: 20px; }
    li { margin-bottom: 5px; }
    blockquote {
      margin: 10px 0;
      padding: 4px 12px;
      border-left: 2px solid #2b3242;
      color: #8d96a8;
    }
    img { max-width: 100%; height: auto; border-radius: 6px; }
    .cp-blocked-asset {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding: 7px 11px;
      border: 1px dashed rgba(245, 158, 11, 0.35);
      border-radius: 6px;
      background: rgba(245, 158, 11, 0.06);
      color: #d6b26f;
      font-family: "JetBrains Mono", ui-monospace, monospace;
      font-size: 11px;
    }
  </style>
</head>
<body>${html}</body>
</html>`;

  useEffect(() => {
    const iframe = iframeRef.current;
    if (!iframe) return;

    const measure = () => {
      try {
        const doc = iframe.contentDocument;
        if (!doc || !doc.body) return;
        const measured = Math.max(doc.body.scrollHeight, doc.documentElement.scrollHeight, 64);
        setHeight(measured + 8);
      } catch {
        // Fallback height if browser restricts measurement
      }
    };

    const handleLoad = () => {
      measure();
      try {
        const doc = iframe.contentDocument;
        if (doc?.body && typeof ResizeObserver !== 'undefined') {
          const ro = new ResizeObserver(measure);
          ro.observe(doc.body);
          iframe.dataset.roAttached = '1';
          return () => ro.disconnect();
        }
      } catch {
        // Safe fallback
      }
    };

    iframe.addEventListener('load', handleLoad);
    return () => {
      iframe.removeEventListener('load', handleLoad);
    };
  }, [srcDoc]);

  return (
    <iframe
      ref={iframeRef}
      title="Sandboxed Email Body"
      sandbox="allow-same-origin"
      referrerPolicy="no-referrer"
      srcDoc={srcDoc}
      style={{ height: `${height}px` }}
      className="w-full border-0 bg-transparent block transition-[height] duration-150"
    />
  );
}
