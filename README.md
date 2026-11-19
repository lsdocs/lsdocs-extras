# lsdocs-extras

Part of [lsdocs](https://github.com/lsdocs), a static-site generator whose
parsers, transformers and renderers are plugins.

`crates/extras` (`lsdocs-extras-plugin`, builtin `extras`): content
features lsdocs adds of its own, neither docutils' nor Sphinx's, in
reStructuredText and Markdown alike: content tabs (`tab-set` /
`tab-item`, sphinx-design's names), which themes show as tabs. It has no
settings.

Its settings and behavior are described in the lsdocs documentation
(https://lsdocs.github.io, "extras").

## Known limitations

- Of sphinx-design's options, `sync-group`, `selected`, `sync`, `name`
  and the class options are accepted and ignored.

## TODO

- [ ] Inline icons (a role).
- [ ] Admonition overrides and additions (new kinds, custom titles and
      icons).
