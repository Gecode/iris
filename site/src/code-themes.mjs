import { ExpressiveCodeTheme } from '@astrojs/starlight/expressive-code';

export const syntaxColours = {
  light: { background: '#F4F8FB', ink: '#172B38', muted: '#4A6170', blue: '#005CA1', orange: '#9B5100', red: '#B71D18', green: '#0B7646', border: '#6D8493' },
  dark: { background: '#102D40', ink: '#EDF4FA', muted: '#BBD0DC', blue: '#8ACBFF', orange: '#EB891B', red: '#FF8F86', green: '#8ED2AA', border: '#628397' },
};

export const irisDark = createTheme('dark');
export const irisLight = createTheme('light');

function createTheme(type) {
  const colour = syntaxColours[type];
  return new ExpressiveCodeTheme({
    name: `iris-${type}`,
    type,
    colors: {
      'editor.background': colour.background,
      'editor.foreground': colour.ink,
      'editorGroupHeader.tabsBackground': colour.background,
      'tab.activeBackground': colour.background,
      'tab.activeForeground': colour.ink,
      'tab.inactiveBackground': colour.background,
      'tab.inactiveForeground': colour.muted,
      'panel.border': colour.border,
      'titleBar.activeBackground': colour.background,
      'titleBar.activeForeground': colour.muted,
      'titleBar.border': colour.border,
      'terminal.background': colour.background,
    },
    tokenColors: [
      { scope: ['variable', 'punctuation'], settings: { foreground: colour.ink } },
      { scope: 'comment', settings: { foreground: colour.muted, fontStyle: 'italic' } },
      { scope: ['keyword', 'storage', 'support.type'], settings: { foreground: colour.blue } },
      { scope: 'string', settings: { foreground: colour.green } },
      { scope: ['constant.numeric', 'constant.language', 'constant.character.escape', 'markup.underline'], settings: { foreground: colour.orange } },
      { scope: ['entity.name', 'support.function'], settings: { foreground: colour.red } },
      { scope: 'invalid', settings: { foreground: colour.red, fontStyle: 'underline' } },
    ],
    styleOverrides: {
      borderColor: colour.border,
      frames: {
        frameBoxShadowCssValue: 'none',
        editorTabBarBorderColor: colour.border,
        inlineButtonForeground: colour.ink,
        inlineButtonBorder: colour.border,
        inlineButtonBorderOpacity: '1',
        inlineButtonBackground: colour.background,
        inlineButtonBackgroundHoverOrFocusOpacity: '1',
        tooltipSuccessBackground: colour.background,
        tooltipSuccessForeground: colour.green,
      },
    },
  });
}
