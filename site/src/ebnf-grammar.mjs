// The EBNF dialect is defined in Specification §2.2.
export const ebnfGrammar = {
  name: 'ebnf',
  scopeName: 'source.ebnf',
  patterns: [
    { name: 'comment.block.ebnf', begin: '\\(\\*', end: '\\*\\)' },
    { name: 'string.quoted.single.ebnf', begin: "'", end: "'" },
    { name: 'string.quoted.double.ebnf', begin: '"', end: '"' },
    { name: 'support.type.ebnf', begin: '\\?', end: '\\?' },
    { name: 'entity.name.function.ebnf', match: '\\b[A-Za-z_][A-Za-z0-9_]*(?=\\s*=)' },
    { name: 'keyword.operator.ebnf', match: '[=|,;{}\\[\\]()]' },
  ],
};
