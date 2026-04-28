//import hljs from "highlight.js/lib/core";

hljs.registerLanguage("eldritch-trace", (hljs) => {
  var FIELDS = [
    'MCRT', 'Emission', 'Detection', 'Material', 'Interface', 'Reflector',
    'Elastic', 'Inelastic', 'Absorption', 'Mie', 'Rayleigh',
    'HenyeyGreenstein', 'Raman', 'Fluorescence', 'Forward', 'Backward',
    'Side', 'Unknown', 'Refraction', 'Reflection', 'ReEmittance', 'Diffuse',
    'Specular', 'Composite', 'SphericalCDF'
  ];
  var SETS = ['any', 'seq', 'perm'];

  var SRC_IDS = ['Mat', 'MatSurf', 'Surf', 'Light', 'Detector'];

  var DECLARATIONS = ['ledger', 'signals', 'src', 'pattern', 'sequence', 'rule'];

  const CHARACTER = {
    className: "string",
    begin: /'([^'\\]|\\.)*'/,
  };
  const STRING = {
    className: "string",
    begin: /"([^"\\]|\\.)*"/,
  };
  const NUMBER = {
    className: "number",
    variants: [
      { begin: /\b0x[0-9A-Fa-f]+\b/ },
      { begin: /\b\d+\b/ }
    ],
    relevance: 0,
  };
  const OPERATOR = {
    className: 'operator',
    begin: /[=+*?!]/
  }
  const SYMBOL = {
    className: 'symbol',
    begin: /\bX\b/
  }

  const COMMENT = {
    variants: [hljs.COMMENT("#", "$")],
  };

  return {
    name: "Eldritch-Trace",
    aliases: ['et', 'eldritch-trace'],
    case_insensitive: false,
    keywords: {
      keyword: DECLARATIONS.concat(SETS),
      type: FIELDS,
      built_in: SRC_IDS
    },
    contains: [
      STRING,
      CHARACTER,
      NUMBER,
      COMMENT,
      OPERATOR,
      SYMBOL,
    ],
  };
});

window.addEventListener("load", (event) => {
  document
    .querySelectorAll('code.language-eldritch-trace, code.language-et')
    .forEach((block) => hljs.highlightBlock(block));
  // document.querySelectorAll('code.language-eldritch-trace, code.language-et').forEach(function(block) {
});

hljs.initHighlightingOnLoad();
