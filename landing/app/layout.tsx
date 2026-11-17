import type { Metadata } from "next";
import "./globals.css";

// Cloudflare Web Analytics: cookieless, no fingerprinting, no consent banner.
// The beacon only renders when a token is present at build time.
const beaconToken = process.env.NEXT_PUBLIC_CF_BEACON_TOKEN;

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
        {/* Plain tag, not next/script: it must land in the exported HTML so the
            first pageview counts before the React bundle runs. */}
        {beaconToken ? (
          <script
            defer
            {...{ fetchpriority: "low", referrerpolicy: "no-referrer" }}
            src="https://static.cloudflareinsights.com/beacon.min.js"
            data-cf-beacon={JSON.stringify({ token: beaconToken })}
          />
        ) : null}
      </head>
      <body>{children}</body>
    </html>
  );
}
