# Deploying the landing page

The page is a static export: `next.config.mjs` sets `output: "export"`, so
`npm run build` writes a plain `out/` folder with no runtime. Cloudflare Pages
serves it for free, and Cloudflare Web Analytics counts visits with no cookies
and no consent banner.

## One-time setup

1. Create a Cloudflare account (free), then **Workers & Pages → Create →
   Pages**. Two ways to connect it:
   - **Connect to Git** (recommended): pick `maheshbvv/vanish`, set
     *Root directory* to `landing`, *Build command* to `npm run build`, *Build
     output directory* to `out`. Add the build watch path `landing/*` so Rust
     commits don't trigger a rebuild.
   - **Upload assets**: drag the local `out/` folder into the dashboard. Simple,
     but you rebuild and re-upload by hand each time.
2. Open **Web Analytics → Add a site**, enter the Pages hostname
   (`<project>.pages.dev`), and copy the token from the beacon snippet.
3. Put that token in the build environment as `NEXT_PUBLIC_CF_BEACON_TOKEN`:
   - Git integration: Pages → Settings → Environment variables.
   - Local build: `cp .env.example .env.local` and fill it in.

Without the token the beacon is omitted entirely — the page ships no analytics
script at all.

## Deploying by hand

```sh
cd landing
npm run build            # reads .env.local, writes out/
npx wrangler pages deploy out --project-name vanish
```

## What you get

Cloudflare Web Analytics reports pageviews, unique visitors, top paths,
referrers, countries, and Core Web Vitals. It sets no cookies and does not
fingerprint, so it agrees with the page's own pitch.

It does not report custom events, so it cannot tell you whether the install
command was copied. For that, ask GitHub how many people actually downloaded a
binary:

```sh
curl -s https://api.github.com/repos/maheshbvv/vanish/releases \
  | jq -r '.[] | .tag_name as $t | .assets[] | "\($t) \(.name) \(.download_count)"'
```

Download counts are cumulative per asset and need no code at all.

## Custom domain

Pages → Custom domains → add the domain. If DNS is already on Cloudflare the
record is created for you; otherwise point a `CNAME` at `<project>.pages.dev`.
