// Captured from the running Vanish binaries on 17 Nov 2026, 10:45.
// Frame glyphs are drawn with ASCII so the page's mono face sets every cell
// (Martian Mono has no box-drawing glyphs). Line content is unedited.

export const hostKick: string[] = [
  "+ Vanish ------------------------------------------------------------------------------+",
  "|* connected  127.0.0.1:8080  ·  you: alice                                            |",
  "+--------------------------------------------------------------------------------------+",
  "  > join key K4YV-7QT2 · invite copied to clipboard",
  "  *** alice joined the chat ***",
  "  *** bob joined the chat ***",
  "10:47 alice   no accounts to make, no database to leak",
  "10:47 bob     one key per boot, and it dies with the room",
  "10:47 alice   nothing is written to disk. not the key, not the messages",
  "  *** bob was removed by the host ***",
  "",
  "+ Message -----------------------------------------------------------------------------+",
  "|>                                                                                     |",
  "+--------------------------------------------------------------------------------------+"
];

// The hero's crop: the same window, with the join notices dropped.
export const hostHero: string[] = hostKick.filter((line) => !/joined the chat/.test(line));

export const guestKick: string[] = [
  "+ Vanish ------------------------------------------------------------------------------+",
  "|* connected  127.0.0.1:8080  ·  you: bob                                              |",
  "+--------------------------------------------------------------------------------------+",
  "  *** bob joined the chat ***",
  "10:47 alice   no accounts to make, no database to leak",
  "10:47 bob     one key per boot, and it dies with the room",
  "10:47 alice   nothing is written to disk. not the key, not the messages",
  "",
  "+ Message -----------------------------------------------------------------------------+",
  "|> Join failed: removed by the host                                                    |",
  "+--------------------------------------------------------------------------------------+"
];

export const guestChat: string[] = [
  "+ Vanish ------------------------------------------------------------------------------+",
  "|* connected  127.0.0.1:8080  ·  you: bob                                              |",
  "+--------------------------------------------------------------------------------------+",
  "  *** bob joined the chat ***",
  "10:47 alice   no accounts to make, no database to leak",
  "10:47 bob     one key per boot, and it dies with the room",
  "10:47 alice   nothing is written to disk. not the key, not the messages",
  "",
  "+ Message -----------------------------------------------------------------------------+",
  "|>                                                                                     |",
  "+--------------------------------------------------------------------------------------+"
];

export const hostChat: string[] = [
  "+ Vanish ------------------------------------------------------------------------------+",
  "|* connected  127.0.0.1:8080  ·  you: alice                                            |",
  "+--------------------------------------------------------------------------------------+",
  "  > join key GDNW-ZGGV",
  "  *** alice joined the chat ***",
  "  *** bob joined the chat ***",
  "10:47 alice   no accounts to make, no database to leak",
  "10:47 bob     one key per boot, and it dies with the room",
  "10:47 alice   nothing is written to disk. not the key, not the messages",
  "",
  "+ Message -----------------------------------------------------------------------------+",
  "|>                                                                                     |",
  "+--------------------------------------------------------------------------------------+"
];

export const serverLines: string[] = [
  "vanish: listening on 127.0.0.1:8080",
  "vanish: join key C7G5-FGB4 (this run only; expires when this server stops)"
];
