# lsdocs-extras

Part of [lsdocs](https://github.com/lsdocs), a static-site generator whose
parsers, transformers and renderers are plugins.

`crates/extras` (`lsdocs-extras-plugin`, builtin `extras`): content
features lsdocs adds of its own, neither docutils' nor Sphinx's, in
reStructuredText and Markdown alike: content tabs (`tab-set` /
`tab-item`, sphinx-design's names), which themes show as tabs, and
buttons (`button-link` / `button-ref`, sphinx-design's names), links
themes draw as buttons. It has no settings.

Its settings and behavior are described in the lsdocs documentation
(https://lsdocs.github.io, "extras").

## Known limitations

- Of sphinx-design's options, `sync-group`, `selected`, `sync`, `name`,
  `click-parent`, `shadow` and the class options are accepted and
  ignored.
- `button-ref` takes a document name with or without its extension
  (without one, the first file of the site with that stem) or a label,
  used as written (Sphinx would also try the label's id form). `ref-type:
  myst` is treated as `any`. A button's content is its text: paragraphs
  are merged, other blocks kept as they are.
- The plugin's cache key covers the site's file list, so adding or
  removing a file recomputes every page's result.

## TODO

- [ ] Inline icons (a role).
- [ ] Admonition overrides and additions (new kinds, custom titles and
      icons).
