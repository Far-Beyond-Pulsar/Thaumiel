/** @type {import('next').NextConfig} */
const nextConfig = {
  // Static export: the whole app is prerendered to plain HTML/JS/CSS files
  // under out/, which crates/thaumiel-ui/build.rs copies for rust-embed to
  // bake into the server binary. No Node runtime ships anywhere -- see
  // crates/thaumiel-ui/README.md.
  output: "export",
  images: { unoptimized: true },
  trailingSlash: false,
};

export default nextConfig;
