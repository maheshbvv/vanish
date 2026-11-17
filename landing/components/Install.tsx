"use client";

import { useEffect, useRef, useState } from "react";

type Layout = { head: string; rest: string | null };

// A command too long for one line is shown on two, never copied that way.
// `a && b && c` breaks at the first `&&`; a pipe breaks before `|`.
function layout(command: string): Layout {
  const andParts = command.split(" && ");
  if (andParts.length > 1) {
    return { head: andParts[0], rest: `&& ${andParts.slice(1).join(" && ")}` };
  }
  const pipe = command.lastIndexOf(" | ");
  if (pipe > 0) {
    return { head: `${command.slice(0, pipe)} \\`, rest: command.slice(pipe + 1) };
  }
  return { head: command, rest: null };
}

export function Install({
  command,
  note,
  id,
}: {
  command: string;
  note?: string;
  id?: string;
}) {
  const [copied, setCopied] = useState(false);
  const [failed, setFailed] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Displayed across two lines; the clipboard gets the single-line command.
  const { head, rest } = layout(command);

  useEffect(() => {
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
  }, []);

  async function copy() {
    setFailed(false);
    try {
      await navigator.clipboard.writeText(command);
      setCopied(true);
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => setCopied(false), 1400);
    } catch {
      setCopied(false);
      setFailed(true);
    }
  }

  return (
    <div className="install" id={id}>
      <div className="install__row">
        <code className="install__cmd">
          <span className="prompt" aria-hidden="true">
            ${" "}
          </span>
          {head}
          {rest ? (
            <>
              {"\n"}
              <span aria-hidden="true">{"  "}</span>
              {rest.trimStart()}
            </>
          ) : null}
        </code>
        <button
          type="button"
          className="install__copy"
          data-copied={copied ? "true" : "false"}
          onClick={copy}
          aria-label="Copy the install command"
        >
          {copied ? "copied" : "copy"}
        </button>
      </div>
      {failed ? (
        <p className="install__note" role="status">
          The clipboard stayed shut. Select the command and copy it yourself.
        </p>
      ) : note ? (
        <p className="install__note">{note}</p>
      ) : null}
    </div>
  );
}
