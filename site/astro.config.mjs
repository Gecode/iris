import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import tailwindcss from '@tailwindcss/vite';
import irisGrammar from '../editors/textmate/iris.tmLanguage.json' with { type: 'json' };
import { irisDark, irisLight } from './src/code-themes.mjs';
import { ebnfGrammar } from './src/ebnf-grammar.mjs';
import { discoverFormatSources } from './scripts/format-sources.mjs';
import { discoverSpecificationSources } from './scripts/specification-sources.mjs';
import { repository } from './scripts/markdown-sources.mjs';

const formats = await discoverFormatSources();
const specifications = await discoverSpecificationSources();

export default defineConfig({
  site: 'https://iris.gecode.dev',
  base: '/',
  output: 'static',
  trailingSlash: 'always',
  vite: {
    worker: { format: 'es' },
    plugins: [tailwindcss()],
    define: { 'import.meta.env.IRIS_REPOSITORY_ROOT': JSON.stringify(repository) },
  },
  integrations: [starlight({
    title: 'Iris',
    description: 'A specification language for structured text formats.',
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
      tabWidth: 0,
      shiki: { langs: [{ ...irisGrammar, name: 'iris', aliases: ['Iris'] }, ebnfGrammar] },
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
      { label: 'Language Specification', collapsed: true, items: specifications.map(({ version, pages }) => ({
        label: `Language version ${version}`, collapsed: true,
        items: pages.map(page => ({ label: page.file === 'README.md' ? 'Chapter index' : page.title, link: page.url })),
      })) },
      { label: 'Formats', collapsed: true, items: [{ label: 'Format library', link: '/formats/' }, ...formats.map(format => ({ label: format.title, link: `/formats/${format.slug}/` }))] },
      { label: 'Using Iris from', collapsed: true, items: ['using/rust', 'using/python', { label: 'Python API reference', link: '/using/python/api/' }, 'using/other-implementations'] },
      { label: 'Tools', collapsed: true, items: ['tools', 'tools/cli', 'tools/formatter', 'tools/lsp', 'tools/editors', 'tools/agent-skill'] },
      { label: 'Playground', link: '/playground/' },
    ],
  })],
});
