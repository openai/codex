# @generated
load("//tools/bzl:genrule2.bzl", "genrule2")

CM_CSS = [
    "lib/codemirror.css",
    "addon/dialog/dialog.css",
    "addon/merge/merge.css",
    "addon/scroll/simplescrollbars.css",
    "addon/search/matchesonscrollbar.css",
    "addon/lint/lint.css",
]

CM_JS = [
    "lib/codemirror.js",
    "mode/meta.js",
    "keymap/emacs.js",
    "keymap/sublime.js",
    "keymap/vim.js",
]

CM_ADDONS = [
    "dialog/dialog.js",
    "edit/closebrackets.js",
    "edit/matchbrackets.js",
    "edit/trailingspace.js",
    "scroll/annotatescrollbar.js",
    "scroll/simplescrollbars.js",
    "search/jump-to-line.js",
    "search/matchesonscrollbar.js",
    "search/searchcursor.js",
    "search/search.js",
    "selection/mark-selection.js",
    "mode/multiplex.js",
    "mode/overlay.js",
    "mode/simple.js",
    "lint/lint.js",
]

# Available themes must be enumerated here,
# in gerrit-extension-api/src/main/java/com/google/gerrit/extensions/client/Theme.java,
# in gerrit-gwtui/src/main/java/net/codemirror/theme/Themes.java
CM_THEMES = [
    "3024-day",
    "3024-night",
    "abcdef",
    "ambiance",
    "base16-dark",
    "base16-light",
    "bespin",
    "blackboard",
    "cobalt",
    "colorforth",
    "dracula",
    "duotone-dark",
    "duotone-light",
    "eclipse",
    "elegant",
    "erlang-dark",
    "hopscotch",
    "icecoder",
    "isotope",
    "lesser-dark",
    "liquibyte",
    "material",
    "mbo",
    "mdn-like",
    "midnight",
    "monokai",
    "neat",
    "neo",
    "night",
    "paraiso-dark",
    "paraiso-light",
    "pastel-on-dark",
    "railscasts",
    "rubyblue",
    "seti",
    "solarized",
    "the-matrix",
    "tomorrow-night-bright",
    "tomorrow-night-eighties",
    "ttcn",
    "twilight",
    "vibrant-ink",
    "xq-dark",
    "xq-light",
    "yeti",
    "zenburn",
]

# Available modes must be enumerated here,
# in gerrit-gwtui/src/main/java/net/codemirror/mode/Modes.java,
# gerrit-gwtui/src/main/java/net/codemirror/mode/ModeInfo.java,
# and in CodeMirror's own mode/meta.js script.
CM_MODES = [
    "apl",
    "asciiarmor",
    "asn.1",
    "asterisk",
    "brainfuck",
    "clike",
    "clojure",
    "cmake",
    "cobol",
    "coffeescript",
    "commonlisp",
    "crystal",
    "css",
    "cypher",
    "d",
    "dart",
    "diff",
    "django",
    "dockerfile",
    "dtd",
    "dylan",
    "ebnf",
    "ecl",
    "eiffel",
    "elm",
    "erlang",
    "factor",
    "fcl",
    "forth",
    "fortran",
    "gas",
    "gfm",
    "gherkin",
    "go",
    "groovy",
    "haml",
    "handlebars",
    "haskell-literate",
    "haskell",
    "haxe",
    "htmlembedded",
    "htmlmixed",
    "http",
    "idl",
    "javascript",
    "jinja2",
    "jsx",
    "julia",
    "livescript",
    "lua",
    "markdown",
    "mathematica",
    "mbox",
    "mirc",
    "mllike",
    "modelica",
    "mscgen",
    "mumps",
    "nginx",
    "nsis",
    "ntriples",
    "octave",
    "oz",
    "pascal",
    "pegjs",
    "perl",
    "php",
    "pig",
    "powershell",
    "properties",
    "protobuf",
    "pug",
    "puppet",
    "python",
    "q",
    "r",
    "rpm",
    "rst",
    "ruby",
    "rust",
    "sas",
    "sass",
    "scheme",
    "shell",
    "sieve",
    "slim",
    "smalltalk",
    "smarty",
    "solr",
    "soy",
    "sparql",
    "spreadsheet",
    "sql",
    "stex",
    "stylus",
    "swift",
    "tcl",
    "textile",
    "tiddlywiki",
    "tiki",
    "toml",
    "tornado",
    "troff",
    "ttcn-cfg",
    "ttcn",
    "turtle",
    "twig",
    "vb",
    "vbscript",
    "velocity",
    "verilog",
    "vhdl",
    "vue",
    "webidl",
    "xml",
    "xquery",
    "yacas",
    "yaml-frontmatter",
    "yaml",
    "z80",
]

CM_VERSION = "5.25.0"

TOP = "META-INF/resources/webjars/codemirror/%s" % CM_VERSION

TOP_MINIFIED = "META-INF/resources/webjars/codemirror-minified/%s" % CM_VERSION

LICENSE = "//lib:LICENSE-codemirror-original"

LICENSE_MINIFIED = "//lib:LICENSE-codemirror-minified"

DIFF_MATCH_PATCH_VERSION = "20121119-1"

DIFF_MATCH_PATCH_TOP = ("META-INF/resources/webjars/google-diff-match-patch/%s" %
                        DIFF_MATCH_PATCH_VERSION)

def pkg_cm():
  for archive, suffix, top, license in [
      ('@codemirror_original//jar', '', TOP, LICENSE),
      ('@codemirror_minified//jar', '_r', TOP_MINIFIED, LICENSE_MINIFIED)
  ]:
    # Main JavaScript and addons
    genrule2(
      name = 'cm' + suffix,
      cmd = ' && '.join([
          "echo '/** @license' >$@",
          'unzip -p $(location %s) %s/LICENSE >>$@' % (archive, top),
          "echo '*/' >>$@",
        ] +
        ['unzip -p $(location %s) %s/%s >>$@' % (archive, top, n) for n in CM_JS] +
        ['unzip -p $(location %s) %s/addon/%s >>$@' % (archive, top, n)
         for n in CM_ADDONS]
      ),
      tools = [archive],
      outs = ['cm%s.js' % suffix],
    )

    # Main CSS
    genrule2(
      name = 'css' + suffix,
      cmd = ' && '.join([
          "echo '/** @license' >$@",
          'unzip -p $(location %s) %s/LICENSE >>$@' % (archive, top),
          "echo '*/' >>$@",
        ] +
        ['unzip -p $(location %s) %s/%s >>$@' % (archive, top, n)
         for n in CM_CSS]
      ),
      tools = [archive],
      outs = ['cm%s.css' % suffix],
    )

    # Modes
    for n in CM_MODES:
      genrule2(
        name = 'mode_%s%s' % (n, suffix),
        cmd = ' && '.join([
            "echo '/** @license' >$@",
            'unzip -p $(location %s) %s/LICENSE >>$@' % (archive, top),
            "echo '*/' >>$@",
            'unzip -p $(location %s) %s/mode/%s/%s.js >>$@' % (archive, top, n, n),
          ]
        ),
        tools = [archive],
        outs = ['mode_%s%s.js' % (n, suffix)],
      )

    # Themes
    for n in CM_THEMES:
      genrule2(
        name = 'theme_%s%s' % (n, suffix),
        cmd = ' && '.join([
            "echo '/** @license' >$@",
            'unzip -p $(location %s) %s/LICENSE >>$@' % (archive, top),
            "echo '*/' >>$@",
            'unzip -p $(location %s) %s/theme/%s.css >>$@' % (archive, top, n)
          ]
        ),
        tools = [archive],
        outs = ['theme_%s%s.css' % (n, suffix)],
      )

    # Merge Addon bundled with diff-match-patch
    genrule2(
      name = 'addon_merge_with_diff_match_patch%s' % suffix,
      cmd = ' && '.join([
          "echo '/** @license' >$@",
          'unzip -p $(location %s) %s/LICENSE >>$@' % (archive, top),
          "echo '*/\n' >>$@",
          "echo '// The google-diff-match-patch library is from https://repo1.maven.org/maven2/org/webjars/google-diff-match-patch/%s/google-diff-match-patch-%s.jar\n' >> $@" % (DIFF_MATCH_PATCH_VERSION, DIFF_MATCH_PATCH_VERSION),
          "echo '/** @license' >>$@",
          "echo 'LICENSE-Apache2.0' >>$@",
          "echo '*/' >>$@",
          'unzip -p $(location @diff_match_patch//jar) %s/diff_match_patch.js >>$@' % DIFF_MATCH_PATCH_TOP,
          "echo ';' >> $@",
          'unzip -p $(location %s) %s/addon/merge/merge.js >>$@' % (archive, top)
        ]
      ),
      tools = [
        '@diff_match_patch//jar',
        # dependency just for license tracking.
        ':diff-match-patch',
        archive,
        "//lib:LICENSE-Apache2.0",
      ],
      outs = ['addon_merge_with_diff_match_patch%s.js' % suffix],
    )

    # Jar packaging
    genrule2(
      name = 'jar' + suffix,
      cmd = ' && '.join([
        'cd $$TMP',
        'mkdir -p net/codemirror/{addon,lib,mode,theme}',
        'cp $$ROOT/$(location :css%s) net/codemirror/lib/cm.css' % suffix,
        'cp $$ROOT/$(location :cm%s) net/codemirror/lib/cm.js' % suffix]
        + ['cp $$ROOT/$(location :mode_%s%s) net/codemirror/mode/%s.js' % (n, suffix, n)
           for n in CM_MODES]
        + ['cp $$ROOT/$(location :theme_%s%s) net/codemirror/theme/%s.css' % (n, suffix, n)
           for n in CM_THEMES]
        + ['cp $$ROOT/$(location :addon_merge_with_diff_match_patch%s) net/codemirror/addon/merge_bundled.js' % suffix]
        + ['zip -qr $$ROOT/$@ net/codemirror/{addon,lib,mode,theme}']),
      tools = [
        ':addon_merge_with_diff_match_patch%s' % suffix,
        ':cm%s' % suffix,
        ':css%s' % suffix,
      ] + [
        ':mode_%s%s' % (n, suffix) for n in CM_MODES
      ] + [
        ':theme_%s%s' % (n, suffix) for n in CM_THEMES
      ],
      outs = ['codemirror%s.jar' % suffix],
    )

    native.java_import(
      name = 'codemirror' + suffix,
      jars = [':jar%s' % suffix],
      visibility = ['//visibility:public'],
      data = [license],
    )
