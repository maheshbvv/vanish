/** @type {import('next').NextConfig} */
const nextConfig = {
  // The landing page is fully static, so `next build` emits a plain `out/`
  // folder that Cloudflare Pages can serve with no runtime.
  output: "export",
};

export default nextConfig;
