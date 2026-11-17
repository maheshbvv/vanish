import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Vanish: a terminal chat that dies with the room",
  description:
    "A self-hosted terminal chat in Rust. One join key per boot, no accounts, no database, and a room that ends when the host quits.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <head>
        <link rel="preconnect" href="https://fonts.googleapis.com" />
        <link rel="preconnect" href="https://fonts.gstatic.com" crossOrigin="anonymous" />
        <link
          rel="stylesheet"
          href="https://fonts.googleapis.com/css2?family=Bricolage+Grotesque:opsz,wdth,wght@12..96,75..100,200..800&family=Martian+Mono:wdth,wght@75..112.5,400..700&display=swap"
        />
      </head>
      <body>{children}</body>
    </html>
  );
}
