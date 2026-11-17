// Captured from the running Vanish binaries on 17 Nov 2026, 08:21.
// Frame glyphs and the product's emoji marks are drawn with ASCII so the page's
// mono face sets every cell (Martian Mono has no box-drawing or emoji glyphs).

export const hostKick: string[] = [
  "+ Vanish ------------------------------------------------------------------------------+",
  "|* connected  127.0.0.1:8080  ·  you: alice                                            |",
  "+--------------------------------------------------------------------------------------+",
  "  * join key RJ4G-1H0M · invite + key copied to clipboard",
  "  * *** alice joined the chat ***",
  "  * *** bob joined the chat ***",
  "08:21 alice   no accounts to make, no database to leak",
  "08:21 bob     one key per boot, and it dies with the room",
  "08:21 alice   nothing is written to disk. not the key, not the messages",
  "  * *** bob left the chat ***",
  "",
  "+ Message -----------------------------------------------------------------------------+",
  "|>                                                                                     |",
  "+--------------------------------------------------------------------------------------+"
];

export const guestKick: string[] = [
  "+ Vanish ------------------------------------------------------------------------------+",
  "|* connected  127.0.0.1:8080  ·  you: bob                                              |",
  "+--------------------------------------------------------------------------------------+",
  "  * *** bob joined the chat ***",
  "08:21 alice   no accounts to make, no database to leak",
  "08:21 bob     one key per boot, and it dies with the room",
  "08:21 alice   nothing is written to disk. not the key, not the messages",
  "",
  "+ Message -----------------------------------------------------------------------------+",
  "|> Join failed: removed by the host                                                    |",
  "+--------------------------------------------------------------------------------------+"
];

export const guestChat: string[] = [
  "+ Vanish ------------------------------------------------------------------------------+",
  "|* connected  127.0.0.1:8080  ·  you: bob                                              |",
  "+--------------------------------------------------------------------------------------+",
  "  * *** bob joined the chat ***",
  "08:21 alice   no accounts to make, no database to leak",
  "08:21 bob     one key per boot, and it dies with the room",
  "08:21 alice   nothing is written to disk. not the key, not the messages",
  "",
  "+ Message -----------------------------------------------------------------------------+",
  "|>                                                                                     |",
  "+--------------------------------------------------------------------------------------+"
];

export const hostChat: string[] = [
  "+ Vanish ------------------------------------------------------------------------------+",
  "|* connected  127.0.0.1:8080  ·  you: alice                                            |",
  "+--------------------------------------------------------------------------------------+",
  "  * join key TWJZ-PMZ2",
  "  * *** alice joined the chat ***",
  "  * *** bob joined the chat ***",
  "08:21 alice   no accounts to make, no database to leak",
  "08:21 bob     one key per boot, and it dies with the room",
  "08:21 alice   nothing is written to disk. not the key, not the messages",
  "",
  "+ Message -----------------------------------------------------------------------------+",
  "|>                                                                                     |",
  "+--------------------------------------------------------------------------------------+"
];

export const serverLines: string[] = [
  "* Vanish server listening on 127.0.0.1:8080",
  "* Join key (this run only): 90X6-RH73   expires when this server stops"
];

// The hero's crop: the same window, with the join notices dropped.
export const hostHero: string[] = hostKick.filter((line) => !/joined the chat/.test(line));
