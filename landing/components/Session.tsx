import type { ReactNode } from "react";

type Line = { kind: string; content: ReactNode };

const TIME = /^(\d{2}:\d{2})(\s+)(\S+)(.*)$/;

function classify(raw: string): Line {
  const line = raw;
  if (line === "") return { kind: "blank", content: " " };

  if (line.startsWith("+")) {
    return { kind: "rule", content: line };
  }

  if (line.startsWith("|")) {
    const inner = line.slice(1);
    const body = inner.replace(/\|$/, "");
    const prompt = body.trimStart().startsWith(">");
    return {
      kind: "body",
      content: (
        <>
          <span className="term__rule">|</span>
          <span className={prompt ? "term__prompt" : undefined}>{body}</span>
          {body.length !== inner.length ? <span className="term__rule">|</span> : null}
        </>
      ),
    };
  }

  if (/(?:[A-Z0-9]{4}-){1}[A-Z0-9]{4}/.test(line)) {
    const parts = line.split(/((?:[A-Z0-9]{4}-){1}[A-Z0-9]{4})/);
    return {
      kind: "key",
      content: (
        <span className="term__muted">
          {parts.map((part, i) =>
            /^(?:[A-Z0-9]{4}-){1}[A-Z0-9]{4}$/.test(part) ? (
              <span className="term__key" key={i}>
                {part}
              </span>
            ) : (
              <span key={i}>{part}</span>
            ),
          )}
        </span>
      ),
    };
  }

  if (line.trimStart().startsWith("* ***")) {
    return { kind: "event", content: <span className="term__muted">{line}</span> };
  }

  const m = line.match(TIME);
  if (m) {
    const [, time, gap, name, rest] = m;
    return {
      kind: "message",
      content: (
        <>
          <span className="term__muted">{time}</span>
          {gap}
          <span className="term__name">{name}</span>
          {rest}
        </>
      ),
    };
  }

  if (line.includes("Join failed")) {
    return { kind: "error", content: <span className="term__name">{line}</span> };
  }

  return { kind: "plain", content: <span className="term__muted">{line}</span> };
}

export function Session({
  lines,
  caption,
  label,
}: {
  lines: string[];
  caption?: string;
  label: string;
}) {
  return (
    <figure className="term">
      <pre tabIndex={0} aria-label={label}>
        <code>
          {lines.map((line, i) => {
            const { kind, content } = classify(line);
            return (
              <span key={i} className={`term__line term__line--${kind}`}>
                {content}
                {"\n"}
              </span>
            );
          })}
        </code>
      </pre>
      {caption ? <figcaption>{caption}</figcaption> : null}
    </figure>
  );
}
