import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import tailwindcss from '@tailwindcss/vite';
import irisGrammar from '../editors/textmate/iris.tmLanguage.json' with { type: 'json' };
import { irisDark, irisLight } from './src/code-themes.mjs';
import { discoverFormatSources } from './scripts/format-sources.mjs';

const formats = await discoverFormatSources();

export default defineConfig({
  site: 'https://iris.gecode.dev',
  base: '/',
  output: 'static',
  trailingSlash: 'always',
  vite: { plugins: [tailwindcss()] },
  integrations: [starlight({
    title: 'Iris',
    description: 'A declarative language for reading text instance files.',
    favicon: '/favicon.svg',
    logo: { src: '../brand/iris-logo-horizontal-dark.svg', alt: 'Iris', replacesTitle: true },
    customCss: ['./src/styles/custom.css', './src/styles/fonts.css'],
    components: { Hero: './src/components/Hero.astro', Header: './src/components/Header.astro', Sidebar: './src/components/Sidebar.astro' },
    head: [
      { tag: 'link', attrs: { rel: 'icon', href: '/favicon.ico', sizes: '16x16 32x32 48x48' } },
      { tag: 'link', attrs: { rel: 'apple-touch-icon', href: '/apple-touch-icon.png', sizes: '180x180' } },
      { tag: 'link', attrs: { rel: 'manifest', href: '/site.webmanifest' } },
      { tag: 'meta', attrs: { name: 'theme-color', content: '#092C43' } },
      { tag: 'meta', attrs: { property: 'og:image', content: 'https://iris.gecode.dev/og-image.png' } },
      { tag: 'meta', attrs: { property: 'og:image:width', content: '1200' } },
      { tag: 'meta', attrs: { property: 'og:image:height', content: '630' } },
      { tag: 'meta', attrs: { property: 'og:image:type', content: 'image/png' } },
      { tag: 'meta', attrs: { name: 'twitter:card', content: 'summary_large_image' } },
      { tag: 'meta', attrs: { name: 'twitter:image', content: 'https://iris.gecode.dev/og-image.png' } },
    ],
    expressiveCode: {
      shiki: { langs: [{ ...irisGrammar, name: 'iris', aliases: ['Iris'] }] },
      themes: [irisDark, irisLight],
      useStarlightDarkModeSwitch: true,
      useStarlightUiThemeColors: false,
      minSyntaxHighlightingColorContrast: 4.5,
      styleOverrides: {
        codeFontFamily: '"Roboto Mono", ui-monospace, monospace',
        uiFontFamily: '"Open Sans", sans-serif',
        borderRadius: '0.25rem',
      },
    },
    sidebar: [
      { label: 'Home', link: '/' },
      { label: 'Introduction', slug: 'introduction' },
      { label: 'Tutorial', collapsed: true, items: ['tutorial', 'tutorial/dimacs', 'tutorial/tsplib', 'tutorial/job-shop'] },
      { label: 'Guides', collapsed: true, items: ['guides', 'guides/patterns', 'guides/sections', 'guides/records-and-checks', 'guides/instances', 'guides/parameters', 'guides/errors', 'guides/performance'] },
      { label: 'Language Specification', collapsed: true, items: [{ label: 'Language version 0.1', slug: 'specification/0.1' }] },
      { label: 'Formats', collapsed: true, items: [{ label: 'Format library', link: '/formats/' }, ...formats.map(format => ({ label: format.title, link: `/formats/${format.slug}/` }))] },
      { label: 'Using Iris from', collapsed: true, items: ['using/rust', 'using/python', 'using/java', 'using/kotlin', 'using/cpp', 'using/gecode'] },
      { label: 'Tools', collapsed: true, items: ['tools', 'tools/cli', 'tools/formatter', 'tools/lsp', 'tools/editors', 'tools/agent-skill'] },
      { label: 'Playground', slug: 'playground' },
    ],
  })],
});
