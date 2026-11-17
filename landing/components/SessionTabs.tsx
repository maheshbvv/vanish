"use client";

import { useId, useRef, useState } from "react";
import { Session } from "./Session";

export type Panel = {
  label: string;
  lines: string[];
  note: string;
  small: string;
  caption: string;
};

export function SessionTabs({ panels }: { panels: Panel[] }) {
  const [active, setActive] = useState(0);
  const base = useId();
  const tabs = useRef<Array<HTMLButtonElement | null>>([]);

  function move(next: number) {
    const index = (next + panels.length) % panels.length;
    setActive(index);
    tabs.current[index]?.focus();
  }

  return (
    <div className="switch">
      <div className="switch__tabs" role="tablist" aria-label="Parts of a session">
        {panels.map((panel, i) => (
          <button
            key={panel.label}
            type="button"
            role="tab"
            id={`${base}-tab-${i}`}
            aria-selected={i === active}
            aria-controls={`${base}-panel-${i}`}
            tabIndex={i === active ? 0 : -1}
            className="tab"
            ref={(node) => {
              tabs.current[i] = node;
            }}
            onClick={() => setActive(i)}
            onKeyDown={(event) => {
              if (event.key === "ArrowRight") {
                event.preventDefault();
                move(active + 1);
              }
              if (event.key === "ArrowLeft") {
                event.preventDefault();
                move(active - 1);
              }
              if (event.key === "Home") {
                event.preventDefault();
                move(0);
              }
              if (event.key === "End") {
                event.preventDefault();
                move(panels.length - 1);
              }
            }}
          >
            {panel.label}
          </button>
        ))}
      </div>
      {panels.map((panel, i) => (
        <div
          key={panel.label}
          role="tabpanel"
          id={`${base}-panel-${i}`}
          aria-labelledby={`${base}-tab-${i}`}
          hidden={i !== active}
          className="switch__body"
        >
          <div className="switch__panel">
            <Session lines={panel.lines} label={panel.caption} caption={panel.caption} />
          </div>
          <div className="switch__note">
            <p>{panel.note}</p>
            <small>{panel.small}</small>
          </div>
        </div>
      ))}
    </div>
  );
}
