import { Install } from "../components/Install";
import { Session } from "../components/Session";
import { SessionTabs } from "../components/SessionTabs";
import { guestChat, guestKick, hostHero } from "../lib/captures";

const COMMAND =
  "curl -fsSL https://raw.githubusercontent.com/maheshbvv/vanish/main/install.sh | sh";
const SOURCE = "https://github.com/maheshbvv/vanish";

const diff = [
  { kind: "head", text: "--- a/what-you-run" },
  { kind: "head", text: "+++ b/what-you-run" },
  { kind: "blank", text: "" },
  { kind: "del", text: " a server kept up between chats" },
  { kind: "del", text: " accounts and password resets" },
  { kind: "del", text: " a database of who said what" },
  { kind: "del", text: " an open inbound port" },
  { kind: "del", text: " chat logs or scrollback traces" },
  { kind: "blank", text: "" },
  { kind: "add", text: " one binary, nothing left running" },
  { kind: "add", text: " a key, random every boot" },
  { kind: "add", text: " a tunnel between machines" },
  { kind: "add", text: " a process that exits with the host" },
  { kind: "add", text: " zero trace on exit, client auto-wiped" },
];

const holds = [
  {
    term: "Join key",
    line: "Eight characters from the OS RNG, never on disk.",
  },
  {
    term: "Guess throttle",
    line: "Five wrong keys in a minute block that address.",
  },
  {
    term: "/kick bob",
    line: "Disconnects the guest, rotates the key.",
  },
  {
    term: "Zero trace",
    line: "Messages live only in RAM. Alternate screen clears terminal scrollback on exit.",
  },
  {
    term: "Self-destruct",
    line: "Guest one-liner auto-deletes on exit. Local clients can pass --vanish to wipe the binary.",
  },
  {
    term: "Transport",
    line: "Plain TCP. tailcat adds WireGuard.",
  },
];

export default function Page() {
  return (
    <>
      <header className="masthead">
        <div className="container masthead__inner">
          <a className="masthead__side" href={SOURCE}>
            source
          </a>
          <a className="masthead__mark" href="#top" aria-label="Vanish, back to the top">
            VANISH
          </a>
          <a className="masthead__side" href="#run">
            run it
          </a>
        </div>
      </header>

      <main id="top">
        <section className="hero">
          <div className="container hero__inner">
            <div className="hero__copy">
              <h1>The room closes when the host quits.</h1>
              <p className="lead">Ephemeral terminal chat in Rust. Completely untraceable, zero trace on exit.</p>
              <Install command={COMMAND} note="Run vanish." />
            </div>
            <Session
              lines={hostHero}
              label="The Vanish host window after kicking a guest, showing the rotated join key"
              caption="Before GDNW-ZGGV, after K4YV-7QT2."
            />
          </div>
        </section>

        <section className="section section--needs">
          <div className="container">
            <div className="section__head">
              <h2>Two binaries, one command.</h2>
            </div>
            <table className="table">
              <thead>
                <tr>
                  <th scope="col">Needs</th>
                  <th scope="col">Does</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td>macOS or Linux</td>
                  <td>Prebuilt for arm64 and x64.</td>
                </tr>
                <tr>
                  <td>A terminal</td>
                  <td>Opens your client in place.</td>
                </tr>
                <tr>
                  <td>Zero-install join</td>
                  <td>Guests join via one-liner; binary auto-purges on disconnect.</td>
                </tr>
                <tr>
                  <td>Zero trace</td>
                  <td>RAM-only. Alternate screen cleans scrollback. Local clients can pass --vanish.</td>
                </tr>
                <tr>
                  <td>tailcat, optional</td>
                  <td>Carries the chat between machines.</td>
                </tr>
                <tr>
                  <td>Clipboard, optional</td>
                  <td>Copies the invite.</td>
                </tr>
              </tbody>
            </table>
          </div>
        </section>

        <section className="section section--tight">
          <div className="container">
            <div className="section__head">
              <h2>Watch the key die.</h2>
            </div>
            <SessionTabs
              panels={[
                {
                  label: "the room",
                  lines: guestChat,
                  note: "A name and the key.",
                  small: "vanish-client",
                  caption: "Guest client.",
                },
                {
                  label: "the kick",
                  lines: guestKick,
                  note: "The key rotates. Bob keeps the old one.",
                  small: "/kick bob",
                  caption: "The removed guest.",
                },
              ]}
            />
          </div>
        </section>

        <section className="section section--diff">
          <div className="container">
            <div className="section__head">
              <h2>What you stop running.</h2>
              <p>The same room, with fewer things to keep alive.</p>
            </div>
            <div className="diff">
              {diff.map((row, i) => (
                <div key={i} className={`diff__row diff__${row.kind}`}>
                  <span aria-hidden="true">{i + 1}</span>
                  <span>{row.text}</span>
                </div>
              ))}
            </div>
          </div>
        </section>

        <section className="section">
          <div className="container holds">
            <div className="holds__head">
              <h2>How the key holds.</h2>
            </div>
            <dl className="holds__list">
              {holds.map((row) => (
                <div className="holds__row" key={row.term}>
                  <dt>{row.term}</dt>
                  <dd>{row.line}</dd>
                </div>
              ))}
            </dl>
          </div>
        </section>

        <section className="band" id="run">
          <div className="container band__inner">
            <div className="band__copy">
              <h2>Start or join.</h2>
              <p>Host a secret room, or join one with zero install.</p>
              <div className="band__links">
                <a href={SOURCE}>source on GitHub</a>
              </div>
            </div>
            <div>
              <p style={{ margin: "0 0 8px 4px", fontSize: "0.85rem", color: "var(--color-muted)" }}>Host a room:</p>
              <Install command={COMMAND} note="Run vanish." />
              <p style={{ margin: "20px 0 8px 4px", fontSize: "0.85rem", color: "var(--color-muted)" }}>Join a room (Zero-trace guest):</p>
              <Install
                command='sh -c "$(curl -fsSL https://raw.githubusercontent.com/maheshbvv/vanish/main/join.sh)" -- <address>'
                note="No install needed. Client auto-deletes when the room closes."
              />
            </div>
          </div>
        </section>
      </main>

      <footer className="footer">
        <div className="container">
          <p>
            Vanish is an ephemeral terminal chat in Rust: tokio on the server, ratatui and crossterm in the client.
            No accounts, no database, no logs, and zero disk trace. All data dies with the room.
            Source at{" "}
            <a href={SOURCE}>github.com/maheshbvv/vanish</a>.
          </p>
        </div>
      </footer>
    </>
  );
}
